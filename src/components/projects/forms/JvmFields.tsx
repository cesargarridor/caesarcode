import { TextInput } from '../../ui/TextInput';
import { Select } from '../../ui/Select';
import { checkArtifact, checkPackage, derivePackage } from '../../../lib/projectValidate';
import { ARCHETYPES, buildOptions, type FormState, type GeneratorId } from '../../../lib/projectRequest';
import type { BuildSystem, JdkInfo } from '../../../lib/types';
import { CheckRow, Field } from './Field';

const BUILD_LABEL: Record<BuildSystem, string> = {
  intellij: 'IntelliJ (sin herramienta de build)',
  maven: 'Maven',
  gradleKotlin: 'Gradle (Kotlin DSL)',
  gradleGroovy: 'Gradle (Groovy DSL)',
};
const JAVA_VERSIONS = [8, 11, 17, 21, 25];

interface Props {
  generator: GeneratorId;
  form: FormState;
  jdks: JdkInfo[];
  onChange: (patch: Partial<FormState>) => void;
  disabled?: boolean;
}

/** Build system, JDK detectado, versión de Java, GroupId/ArtifactId/paquete y código de muestra. */
export function JvmFields({ generator, form, jdks, onChange, disabled }: Props) {
  const builds = buildOptions(generator);
  const build = builds.includes(form.build) ? form.build : builds[0];
  const jdkOptions = [
    { value: '', label: jdks.length ? 'Automático (PATH / JAVA_HOME)' : 'No se detectó ningún JDK' },
    ...jdks.map((j) => ({ value: j.home, label: `${j.version}${j.vendor ? ` · ${j.vendor}` : ''}` })),
  ];
  const javaVersions = JAVA_VERSIONS.includes(form.javaVersion) ? JAVA_VERSIONS : [...JAVA_VERSIONS, form.javaVersion].sort((a, b) => a - b);
  const setJdk = (home: string) => {
    const j = jdks.find((x) => x.home === home);
    onChange({ jdkHome: home, ...(j ? { javaVersion: j.major } : {}) });
  };
  const setGroup = (groupId: string) =>
    onChange({ groupId, ...(form.packageTouched ? {} : { packageName: derivePackage(groupId, form.artifactId) }) });
  const setArtifact = (artifactId: string) =>
    onChange({ artifactId, artifactTouched: true, ...(form.packageTouched ? {} : { packageName: derivePackage(form.groupId, artifactId) }) });
  const sample = generator === 'java' || generator === 'kotlin' || generator === 'groovy';
  const isArchetype = generator === 'mavenArchetype';
  const customArchetype = isArchetype && !ARCHETYPES[form.archetype];

  return (
    <div className="space-y-2.5">
      <Field label="Build system">
        <Select value={build} options={builds.map((b) => ({ value: b, label: BUILD_LABEL[b] }))} onChange={(v) => onChange({ build: v })} ariaLabel="Build system" />
      </Field>
      {isArchetype && (
        <>
          <Field label="Archetype">
            <Select
              value={form.archetype}
              options={[...Object.entries(ARCHETYPES).map(([k, a]) => ({ value: k, label: a.label })), { value: 'custom', label: 'Personalizado…' }]}
              onChange={(archetype) => onChange({ archetype })}
              ariaLabel="Archetype"
            />
          </Field>
          {customArchetype && (
            <>
              <Field label="Archetype groupId"><TextInput value={form.archGroup} disabled={disabled} onChange={(e) => onChange({ archGroup: e.target.value })} /></Field>
              <Field label="Archetype artifactId"><TextInput value={form.archArtifact} disabled={disabled} onChange={(e) => onChange({ archArtifact: e.target.value })} /></Field>
              <Field label="Archetype version"><TextInput value={form.archVersion} disabled={disabled} onChange={(e) => onChange({ archVersion: e.target.value })} /></Field>
            </>
          )}
        </>
      )}
      <Field label="JDK">
        <Select value={form.jdkHome} options={jdkOptions} onChange={setJdk} ariaLabel="JDK" />
      </Field>
      <Field label="Java version">
        <Select value={String(form.javaVersion)} options={javaVersions.map((v) => ({ value: String(v), label: String(v) }))} onChange={(v) => onChange({ javaVersion: Number(v) })} ariaLabel="Java version" />
      </Field>
      <Field label="GroupId" error={checkPackage(form.groupId)}>
        <TextInput value={form.groupId} disabled={disabled} onChange={(e) => setGroup(e.target.value)} aria-label="GroupId" />
      </Field>
      <Field label="ArtifactId" error={form.artifactId ? checkArtifact(form.artifactId) : null}>
        <TextInput value={form.artifactId} disabled={disabled} onChange={(e) => setArtifact(e.target.value)} aria-label="ArtifactId" />
      </Field>
      <Field label="Package" error={form.packageName ? checkPackage(form.packageName) : null}>
        <TextInput value={form.packageName} disabled={disabled} onChange={(e) => onChange({ packageName: e.target.value, packageTouched: true })} aria-label="Package" />
      </Field>
      {(generator === 'quarkus' || generator === 'micronaut') && (
        <Field label={generator === 'quarkus' ? 'Extensions' : 'Features'} hint="Ids separados por coma (opcional)">
          <TextInput value={form.extras} disabled={disabled} onChange={(e) => onChange({ extras: e.target.value })} placeholder={generator === 'quarkus' ? 'rest-jackson, hibernate-orm-panache' : 'data-jdbc, postgres'} />
        </Field>
      )}
      {sample && <CheckRow label="Add sample code" checked={form.sampleCode} disabled={disabled} onChange={(sampleCode) => onChange({ sampleCode })} />}
    </div>
  );
}
