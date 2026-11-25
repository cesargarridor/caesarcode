import { describe, expect, it } from 'vitest';
import { buildRequest, defaultForm, defaultSpring, validateForm, validateSpring, withName } from './projectRequest';

const base = () => ({ ...withName(defaultForm('/tmp'), 'My-App'), location: '/tmp' });

describe('projectRequest', () => {
  it('deriva artifactId y paquete del nombre hasta que se tocan', () => {
    const f = base();
    expect(f.artifactId).toBe('my-app');
    expect(f.packageName).toBe('com.example.myapp');
    const t = withName({ ...f, packageTouched: true, packageName: 'org.x' }, 'Other');
    expect(t.packageName).toBe('org.x');
    expect(t.artifactId).toBe('other');
  });

  it('construye peticiones JVM con el contrato de Rust', () => {
    const r = buildRequest('java', base(), defaultSpring('x'));
    expect(r).toMatchObject({ generator: 'java', common: { name: 'My-App', location: '/tmp', git: true }, jvm: { build: 'maven', groupId: 'com.example', artifactId: 'my-app', packageName: 'com.example.myapp', jdkHome: null } });
    const k = buildRequest('ktor', { ...base(), build: 'maven' }, defaultSpring('x'));
    expect(k).toMatchObject({ generator: 'ktor', jvm: { build: 'gradleKotlin' } });
  });

  it('arquetipo de la lista corta y personalizado', () => {
    const r = buildRequest('mavenArchetype', base(), defaultSpring('x'));
    expect(r).toMatchObject({ archetype: { artifact: 'maven-archetype-quickstart' } });
    const c = buildRequest('mavenArchetype', { ...base(), archetype: 'custom', archGroup: 'a.b', archArtifact: 'c', archVersion: '1' }, defaultSpring('x'));
    expect(c).toMatchObject({ archetype: { group: 'a.b', artifact: 'c', version: '1' } });
  });

  it('Spring Boot simple = microservicio único', () => {
    const s = { ...defaultSpring('shop'), gateway: { on: true, port: 8080 } };
    const r = buildRequest('springBoot', { ...base(), name: 'orders-service' }, s);
    expect(r).toMatchObject({ generator: 'springMicroservices', spring: { single: true, gateway: null, discovery: null } });
    const m = buildRequest('springMicroservices', base(), s);
    expect(m).toMatchObject({ spring: { single: false, gateway: { port: 8080 } } });
  });

  it('valida formulario y Spring', () => {
    expect(validateForm('java', base(), defaultSpring('x'))).toEqual([]);
    expect(validateForm('java', { ...base(), name: '../x' }, defaultSpring('x')).length).toBeGreaterThan(0);
    expect(validateForm('java', { ...base(), location: '' }, defaultSpring('x'))).toContainEqual(expect.stringContaining('Location'));
    expect(validateForm('angular', { ...base(), name: 'my.app' }, defaultSpring('x'))).toContainEqual(expect.stringContaining('Angular'));
    const s = defaultSpring('x');
    s.services.push({ key: 'b', name: 'users-service', port: 8081, deps: [], layers: true });
    expect(validateSpring(s, false)).toContainEqual(expect.stringContaining('8081'));
    s.services[1].port = 8082;
    s.gateway = { on: true, port: 8082 };
    expect(validateSpring(s, false).some((e) => e.includes('8082'))).toBe(true);
    s.gateway.port = 8080;
    expect(validateSpring(s, false)).toEqual([]);
    s.services[1].name = 'api-gateway';
    expect(validateSpring(s, false).some((e) => e.includes('reservado'))).toBe(true);
  });
});
