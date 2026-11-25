import { describe, expect, it } from 'vitest';
import { qualifyEphemeral, qualifyId, repoFor, rootFor } from './paths';

const roots = [
  { path: '/w/main', name: 'main' },
  { path: '/w/api', name: 'api' },
  { path: '/w/api-v2', name: 'api-v2' },
];

describe('rootFor', () => {
  it('devuelve la raíz que contiene la ruta', () => {
    expect(rootFor(roots, '/w/api/src/a.ts')?.name).toBe('api');
    expect(rootFor(roots, '/w/main')?.name).toBe('main');
  });
  it('no confunde prefijos de nombre (api vs api-v2)', () => {
    expect(rootFor(roots, '/w/api-v2/x')?.name).toBe('api-v2');
  });
  it('null fuera de las raíces', () => {
    expect(rootFor(roots, '/otro/x')).toBeNull();
  });
  it('acepta separadores de Windows', () => {
    expect(rootFor([{ path: 'C:\\w\\api', name: 'api' }], 'C:\\w\\api\\src\\a.ts')?.name).toBe('api');
  });
});

describe('repoFor', () => {
  const repos = [{ top: '/w/main' }, { top: '/w/main/libs/core' }, { top: '/w/api' }];
  it('elige el top más largo (repo anidado)', () => {
    expect(repoFor(repos, '/w/main/libs/core/a.ts')?.top).toBe('/w/main/libs/core');
    expect(repoFor(repos, '/w/main/src/a.ts')?.top).toBe('/w/main');
  });
  it('null si ningún repo lo contiene', () => {
    expect(repoFor(repos, '/w/zzz/a.ts')).toBeNull();
  });
});

describe('qualifyId / qualifyEphemeral', () => {
  it('la primaria conserva el id crudo', () => {
    expect(qualifyId(roots, '/w/main', 'web')).toBe('web');
    expect(qualifyEphemeral(roots, '/w/main', 'run', 'dev')).toBe('run:dev');
  });
  it('las demás llevan el nombre de raíz', () => {
    expect(qualifyId(roots, '/w/api', 'web')).toBe('api/web');
    expect(qualifyEphemeral(roots, '/w/api', 'file', 'src/x.js')).toBe('file:api/src/x.js');
  });
});
