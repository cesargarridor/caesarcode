import { describe, expect, it } from 'vitest';
import { classifyFolder } from './classifyFolder';
import { palette } from './palette';

describe('classifyFolder', () => {
  it('source', () => {
    const s = classifyFolder('src', '/p/src');
    expect(s.cls).toBe('source');
    expect(s.color).toBe('#38bdf8');
  });
  it('test y case-insensitive', () => {
    expect(classifyFolder('tests', '/p/tests').cls).toBe('test');
    expect(classifyFolder('Tests', '/p/Tests').color).toBe(palette.green);
  });
  it('config e infra', () => {
    expect(classifyFolder('resources', '/p/resources').color).toBe('#eab308');
    const i = classifyFolder('.github', '/p/.github');
    expect(i.cls).toBe('infra');
    expect(i.color).toBe('#a78bfa');
  });
  it('excluded atenuada y colapsada', () => {
    const s = classifyFolder('target', '/p/target');
    expect(s.cls).toBe('excluded');
    expect(s.dimName).toBe(true);
    expect(s.collapsedByDefault).toBe(true);
    expect(s.color).toBe('#52525b');
  });
  it('excluded gana a module', () => {
    expect(classifyFolder('node_modules', '/p/node_modules', ['package.json']).cls).toBe('excluded');
    expect(classifyFolder('dist', '/p/dist', null, { module: 'node' }).cls).toBe('excluded');
  });
  it('module por children con badge', () => {
    const s = classifyFolder('api', '/p/api', ['pom.xml', 'src']);
    expect(s.cls).toBe('module');
    expect(s.badge).toBe('mvn');
    expect(s.color).toBe('#f59e0b');
  });
  it('module por opts.module sin children', () => {
    expect(classifyFolder('web', '/p/web', undefined, { module: 'node' }).badge).toBe('npm');
    expect(classifyFolder('svc', '/p/svc', null, { module: 'cargo' }).badge).toBe('cargo');
  });
  it('root con marcador', () => {
    const s = classifyFolder('proj', '/proj', ['go.mod'], { isRoot: true });
    expect(s.cls).toBe('module');
    expect(s.badge).toBe('go');
  });
  it('module gana a source/test', () => {
    expect(classifyFolder('src', '/p/x/src', ['package.json']).cls).toBe('module');
  });
  it('plain', () => {
    const s = classifyFolder('docs', '/p/docs');
    expect(s.cls).toBe('plain');
    expect(s.dimName).toBe(false);
  });
  it('el rol de Rust gana a la heurística', () => {
    const s = classifyFolder('java', '/p/src/main/java', undefined, { role: 'sourceRoot' });
    expect(s.cls).toBe('source');
    expect(s.role).toBe('sourceRoot');
    expect(s.color).toBe('#548AF7');
    expect(classifyFolder('java', '/p/src/test/java', undefined, { role: 'testRoot' }).cls).toBe('test');
    expect(classifyFolder('resources', '/p/r', undefined, { role: 'resourcesRoot' }).color).toBe('#E5A541');
    expect(classifyFolder('com', '/p/com', undefined, { role: 'package' }).cls).toBe('package');
    // src con rol de paquete deja de ser "source" por nombre
    expect(classifyFolder('src', '/p/x/src', undefined, { role: 'package' }).cls).toBe('package');
  });
  it('rol excluded atenúa aunque el nombre no esté en la lista', () => {
    const s = classifyFolder('.gradle', '/p/.gradle', undefined, { role: 'excluded' });
    expect(s.cls).toBe('excluded');
    expect(s.dimName).toBe(true);
  });
  it('rol module conserva badge y la caché distingue por rol', () => {
    const a = classifyFolder('svc', '/p/svc', ['pom.xml'], { role: 'module' });
    expect(a.cls).toBe('module');
    expect(a.badge).toBe('mvn');
    const b = classifyFolder('svc', '/p/svc', ['pom.xml'], { role: 'excluded' });
    expect(b.cls).toBe('excluded');
  });
});
