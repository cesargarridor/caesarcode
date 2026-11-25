import { describe, expect, it } from 'vitest';
import { templatesFor } from './liveTemplates';

const labels = (p: string) => templatesFor(p).map((c) => c.label);

describe('liveTemplates', () => {
  it('java', () => {
    for (const l of ['getm', 'postm', 'putm', 'delm', 'restc', 'serv', 'logi', 'loge', 'tryc', 'testm', 'lambdah'])
      expect(labels('A.java')).toContain(l);
  });
  it('python', () => {
    for (const l of ['main', 'tryc', 'lambdap']) expect(labels('a.py')).toContain(l);
  });
  it('ts/js', () => {
    expect(labels('a.ts')).toContain('tryc');
    expect(labels('a.js')).toContain('tryc');
  });
  it('yaml', () => {
    expect(labels('application.yml')).toContain('springapp');
    expect(labels('x.yaml')).toContain('springapp');
  });
  it('otros: vacío; todas son snippet', () => {
    expect(labels('a.rs')).toEqual([]);
    for (const c of templatesFor('A.java')) expect(c.type).toBe('snippet');
  });
});
