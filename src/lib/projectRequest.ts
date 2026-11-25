// Estado del formulario del asistente New Project → `ProjectRequest` (contrato de Rust) + validación.

import type { BuildSystem, ProjectRequest, SpringRequest } from './types';
import {
  checkArtifact, checkBootVersion, checkGoModule, checkPackage, checkPort, checkProjectName, checkServiceName,
  deriveArtifact, derivePackage, pkgSegment, type Check,
} from './projectValidate';

export const INFRA_NAMES = { gateway: 'api-gateway', discovery: 'eureka-server', config: 'config-server' } as const;

export interface FormState {
  name: string;
  location: string;
  git: boolean;
  build: BuildSystem;
  javaVersion: number;
  jdkHome: string;
  groupId: string;
  artifactId: string;
  packageName: string;
  /** Mientras no se toquen, artifactId/paquete se derivan del nombre. */
  artifactTouched: boolean;
  packageTouched: boolean;
  sampleCode: boolean;
  typescript: boolean;
  router: boolean;
  pinia: boolean;
  viteTemplate: string;
  ngRouting: boolean;
  ngStyle: string;
  venv: boolean;
  goModule: string;
  goModuleTouched: boolean;
  rustBin: boolean;
  archetype: string;
  archGroup: string;
  archArtifact: string;
  archVersion: string;
  /** Extensiones de Quarkus / features de Micronaut (ids separados por coma o espacio). */
  extras: string;
}

export interface ServiceForm {
  key: string;
  name: string;
  port: number;
  deps: string[];
  layers: boolean;
}
export interface InfraForm { on: boolean; port: number }
export interface SpringForm {
  groupId: string;
  basePackage: string;
  basePackageTouched: boolean;
  javaVersion: number;
  bootVersion: string | null;
  build: BuildSystem;
  services: ServiceForm[];
  gateway: InfraForm;
  discovery: InfraForm;
  config: InfraForm;
  dockerCompose: boolean;
  dockerfiles: boolean;
  preferOnline: boolean;
}

export const ARCHETYPES: Record<string, { group: string; artifact: string; version: string; label: string }> = {
  quickstart: { group: 'org.apache.maven.archetypes', artifact: 'maven-archetype-quickstart', version: '1.5', label: 'quickstart' },
  webapp: { group: 'org.apache.maven.archetypes', artifact: 'maven-archetype-webapp', version: '1.5', label: 'webapp' },
  simple: { group: 'org.apache.maven.archetypes', artifact: 'maven-archetype-simple', version: '1.5', label: 'simple' },
};

export const VITE_TEMPLATES = [
  'vanilla', 'vanilla-ts', 'react', 'react-ts', 'vue', 'vue-ts', 'svelte', 'svelte-ts', 'preact', 'preact-ts',
  'lit', 'lit-ts', 'solid', 'solid-ts',
];
export const ANGULAR_STYLES = ['css', 'scss', 'sass', 'less'];

export const SPRING_PRESETS = [
  'web', 'data-jpa', 'postgresql', 'mysql', 'security', 'validation', 'lombok', 'actuator', 'kafka', 'amqp',
  'data-redis', 'cloud-feign', 'cloud-eureka', 'cloud-config-client', 'devtools', 'webflux', 'data-mongodb',
];

export function defaultForm(location = ''): FormState {
  return {
    name: '', location, git: true, build: 'maven', javaVersion: 21, jdkHome: '',
    groupId: 'com.example', artifactId: '', packageName: '', artifactTouched: false, packageTouched: false,
    sampleCode: true, typescript: true, router: false, pinia: false, viteTemplate: 'react-ts',
    ngRouting: true, ngStyle: 'css', venv: true, goModule: '', goModuleTouched: false, rustBin: true,
    archetype: 'quickstart', archGroup: '', archArtifact: '', archVersion: '', extras: '',
  };
}

/** Aplica el nombre y re-deriva lo no tocado. */
export function withName(f: FormState, name: string): FormState {
  const next = { ...f, name };
  const artifact = f.artifactTouched ? f.artifactId : deriveArtifact(name);
  next.artifactId = artifact;
  if (!f.packageTouched) next.packageName = derivePackage(f.groupId, artifact);
  if (!f.goModuleTouched) next.goModule = name.toLowerCase().replace(/[^a-z0-9._-]+/g, '-');
  return next;
}

export function nextKey(): string {
  return Math.random().toString(36).slice(2, 10);
}

export function defaultSpring(name: string): SpringForm {
  return {
    groupId: 'com.example',
    basePackage: derivePackage('com.example', deriveArtifact(name || 'shop')),
    basePackageTouched: false,
    javaVersion: 21,
    bootVersion: null,
    build: 'maven',
    services: [{ key: nextKey(), name: 'orders-service', port: 8081, deps: ['web', 'actuator'], layers: true }],
    gateway: { on: false, port: 8080 },
    discovery: { on: false, port: 8761 },
    config: { on: false, port: 8888 },
    dockerCompose: true,
    dockerfiles: true,
    preferOnline: true,
  };
}

const splitIds = (s: string) => s.split(/[\s,]+/).map((x) => x.trim()).filter(Boolean);

export type GeneratorId =
  | 'java' | 'kotlin' | 'groovy' | 'empty' | 'mavenArchetype' | 'javaFx' | 'jakartaEe' | 'springBoot'
  | 'springMicroservices' | 'quarkus' | 'micronaut' | 'ktor' | 'html' | 'react' | 'vue' | 'vite' | 'angular'
  | 'express' | 'node' | 'python' | 'go' | 'rust';

const JVM_GENERATORS: GeneratorId[] = ['java', 'kotlin', 'groovy', 'javaFx', 'jakartaEe', 'ktor', 'mavenArchetype', 'quarkus', 'micronaut'];
export const isJvm = (g: GeneratorId) => JVM_GENERATORS.includes(g);

/** Sistemas de compilación ofrecidos por generador. */
export function buildOptions(g: GeneratorId): BuildSystem[] {
  switch (g) {
    case 'java': return ['intellij', 'maven', 'gradleKotlin', 'gradleGroovy'];
    case 'kotlin': case 'groovy': return ['gradleKotlin', 'gradleGroovy', 'maven'];
    case 'quarkus': case 'micronaut': return ['maven', 'gradleKotlin', 'gradleGroovy'];
    case 'ktor': return ['gradleKotlin'];
    default: return ['maven'];
  }
}

function jvmOpts(g: GeneratorId, f: FormState) {
  const allowed = buildOptions(g);
  return {
    build: allowed.includes(f.build) ? f.build : allowed[0],
    javaVersion: f.javaVersion,
    jdkHome: f.jdkHome || null,
    groupId: f.groupId,
    artifactId: f.artifactId,
    packageName: f.packageName,
    sampleCode: f.sampleCode,
  };
}

export function springRequest(s: SpringForm, single: boolean): SpringRequest {
  return {
    groupId: s.groupId,
    basePackage: s.basePackage,
    javaVersion: s.javaVersion,
    bootVersion: s.bootVersion,
    build: s.build,
    services: s.services.map((x) => ({ name: x.name, port: x.port, dependencies: x.deps, layers: x.layers })),
    gateway: s.gateway.on ? { port: s.gateway.port } : null,
    discovery: s.discovery.on ? { port: s.discovery.port } : null,
    configServer: s.config.on ? { port: s.config.port } : null,
    dockerCompose: s.dockerCompose,
    dockerfiles: s.dockerfiles,
    preferOnline: s.preferOnline,
    single,
  };
}

/** Spring Boot simple: un único servicio (con el nombre del proyecto) y sin infraestructura. */
export function singleSpring(s: SpringForm, projectName: string): SpringForm {
  const first = s.services[0] ?? { key: nextKey(), name: '', port: 8080, deps: ['web'], layers: true };
  return {
    ...s,
    services: [{ ...first, name: deriveArtifact(projectName), port: first.port }],
    gateway: { ...s.gateway, on: false },
    discovery: { ...s.discovery, on: false },
    config: { ...s.config, on: false },
  };
}

/** Spring Boot (un servicio) se envía como `springMicroservices` con `single`. */
export function buildRequest(g: GeneratorId, f: FormState, s: SpringForm): ProjectRequest {
  const common = { name: f.name, location: f.location, git: f.git };
  switch (g) {
    case 'java': case 'kotlin': case 'groovy': case 'javaFx': case 'jakartaEe': case 'ktor':
      return { generator: g, common, jvm: jvmOpts(g, f) };
    case 'mavenArchetype': {
      const a = ARCHETYPES[f.archetype];
      const archetype = a ? { group: a.group, artifact: a.artifact, version: a.version } : { group: f.archGroup, artifact: f.archArtifact, version: f.archVersion };
      return { generator: 'mavenArchetype', common, jvm: jvmOpts(g, f), archetype };
    }
    case 'quarkus': return { generator: 'quarkus', common, jvm: jvmOpts(g, f), extensions: splitIds(f.extras) };
    case 'micronaut': return { generator: 'micronaut', common, jvm: jvmOpts(g, f), features: splitIds(f.extras) };
    case 'springMicroservices': return { generator: 'springMicroservices', common, spring: springRequest(s, false) };
    case 'springBoot': {
      return { generator: 'springMicroservices', common, spring: springRequest(singleSpring(s, f.name), true) };
    }
    case 'empty': case 'html': return { generator: g, common };
    case 'express': case 'node': case 'react': return { generator: g, common, typescript: f.typescript };
    case 'vue': return { generator: 'vue', common, typescript: f.typescript, router: f.router, pinia: f.pinia };
    case 'vite': return { generator: 'vite', common, template: f.viteTemplate };
    case 'angular': return { generator: 'angular', common, routing: f.ngRouting, style: f.ngStyle };
    case 'python': return { generator: 'python', common, venv: f.venv };
    case 'go': return { generator: 'go', common, module: f.goModule };
    case 'rust': return { generator: 'rust', common, bin: f.rustBin };
  }
}

export function validateSpring(s: SpringForm, single: boolean): string[] {
  const errs: string[] = [];
  const push = (c: Check, who: string) => { if (c) errs.push(`${who}: ${c}`); };
  push(checkPackage(s.groupId), 'GroupId');
  push(checkPackage(s.basePackage), 'Paquete base');
  if (s.bootVersion) push(checkBootVersion(s.bootVersion), 'Spring Boot');
  if (s.services.length === 0) errs.push('Añade al menos un microservicio');
  const names = new Set<string>();
  const ports = new Map<number, string>();
  const claim = (port: number, who: string) => {
    push(checkPort(port), `Puerto de ${who}`);
    const other = ports.get(port);
    if (other) errs.push(`Puerto ${port} repetido (${who} y ${other})`);
    else ports.set(port, who);
  };
  for (const svc of s.services) {
    push(single ? checkProjectName(svc.name) : checkServiceName(svc.name), `Servicio "${svc.name}"`);
    if (names.has(svc.name)) errs.push(`Servicio duplicado: ${svc.name}`);
    names.add(svc.name);
    if (!checkServiceName(svc.name)) push(checkPackage(`${s.basePackage}.${pkgSegment(svc.name)}`), `Paquete de ${svc.name}`);
    claim(svc.port, svc.name);
  }
  if (!single) {
    for (const [k, label] of [['gateway', INFRA_NAMES.gateway], ['discovery', INFRA_NAMES.discovery], ['config', INFRA_NAMES.config]] as const) {
      if (!s[k].on) continue;
      if (names.has(label)) errs.push(`El nombre "${label}" está reservado para la infraestructura`);
      claim(s[k].port, label);
    }
  }
  return errs;
}

/** Errores bloqueantes del formulario (vacío = se puede crear). */
export function validateForm(g: GeneratorId, f: FormState, s: SpringForm): string[] {
  const errs: string[] = [];
  const push = (c: Check, who: string) => { if (c) errs.push(`${who}: ${c}`); };
  push(checkProjectName(f.name), 'Nombre');
  if (!f.location.trim()) errs.push('Location: elige la carpeta de destino');
  if (isJvm(g)) {
    push(checkPackage(f.groupId), 'GroupId');
    push(checkArtifact(f.artifactId), 'ArtifactId');
    push(checkPackage(f.packageName), 'Paquete');
  }
  if (g === 'mavenArchetype' && !ARCHETYPES[f.archetype]) {
    for (const [v, who] of [[f.archGroup, 'Archetype groupId'], [f.archArtifact, 'Archetype artifactId'], [f.archVersion, 'Archetype versión']] as const) {
      if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,100}$/.test(v)) errs.push(`${who}: no válido`);
    }
  }
  if ((g === 'quarkus' || g === 'micronaut') && splitIds(f.extras).some((x) => !/^[A-Za-z0-9][A-Za-z0-9._:-]{0,100}$/.test(x))) {
    errs.push('Extensiones/features: ids no válidos');
  }
  if (g === 'go') push(checkGoModule(f.goModule), 'Módulo');
  if (g === 'angular' && !/^[A-Za-z][A-Za-z0-9-]*$/.test(f.name)) errs.push('Nombre: Angular solo admite letras, dígitos y "-"');
  if (g === 'springMicroservices' || g === 'springBoot') {
    const single = g === 'springBoot';
    errs.push(...validateSpring(single ? singleSpring(s, f.name) : s, single));
  }
  return errs;
}

/** Metadatos de presentación (el backend aporta disponibilidad/estrategia vía `project_generators`). */
export const GENERATOR_META: { id: GeneratorId; label: string; group: string }[] = [
  { id: 'java', label: 'Java', group: 'Java/JVM' },
  { id: 'kotlin', label: 'Kotlin', group: 'Java/JVM' },
  { id: 'groovy', label: 'Groovy', group: 'Java/JVM' },
  { id: 'empty', label: 'Empty Project', group: 'Java/JVM' },
  { id: 'mavenArchetype', label: 'Maven Archetype', group: 'Java/JVM' },
  { id: 'javaFx', label: 'JavaFX', group: 'Java/JVM' },
  { id: 'jakartaEe', label: 'Jakarta EE', group: 'Java/JVM' },
  { id: 'springBoot', label: 'Spring Boot', group: 'Frameworks' },
  { id: 'springMicroservices', label: 'Spring Microservices', group: 'Frameworks' },
  { id: 'quarkus', label: 'Quarkus', group: 'Frameworks' },
  { id: 'micronaut', label: 'Micronaut', group: 'Frameworks' },
  { id: 'ktor', label: 'Ktor', group: 'Frameworks' },
  { id: 'html', label: 'HTML', group: 'Web' },
  { id: 'react', label: 'React', group: 'Web' },
  { id: 'vue', label: 'Vue.js', group: 'Web' },
  { id: 'vite', label: 'Vite', group: 'Web' },
  { id: 'angular', label: 'Angular CLI', group: 'Web' },
  { id: 'express', label: 'Express', group: 'Web' },
  { id: 'node', label: 'Node.js', group: 'Web' },
  { id: 'python', label: 'Python', group: 'Otros' },
  { id: 'go', label: 'Go', group: 'Otros' },
  { id: 'rust', label: 'Rust', group: 'Otros' },
];
