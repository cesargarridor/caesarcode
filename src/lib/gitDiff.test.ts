import { describe, expect, it } from 'vitest';
import { diffStats, hunkPatch } from './gitDiff';
import type { FileDiff } from './types';

const HEADER = 'diff --git a/f.txt b/f.txt\nindex 111..222 100644\n--- a/f.txt\n+++ b/f.txt\n';
const diff: FileDiff = {
  path: 'f.txt', oldPath: null, binary: false, tooLarge: false, header: HEADER,
  hunks: [
    {
      header: '@@ -1,3 +1,3 @@ fn x', oldStart: 1, oldLines: 3, newStart: 1, newLines: 3,
      lines: [
        { kind: ' ', text: 'a', oldNo: 1, newNo: 1 },
        { kind: '-', text: 'b', oldNo: 2, newNo: null },
        { kind: '+', text: 'B', oldNo: null, newNo: 2 },
        { kind: ' ', text: 'c', oldNo: 3, newNo: 3 },
      ],
    },
    {
      header: '@@ -10,2 +10,3 @@', oldStart: 10, oldLines: 2, newStart: 10, newLines: 3,
      lines: [
        { kind: ' ', text: 'x', oldNo: 10, newNo: 10 },
        { kind: '+', text: 'y', oldNo: null, newNo: 11 },
        { kind: ' ', text: 'z', oldNo: 11, newNo: 12 },
        { kind: '\\', text: 'No newline at end of file', oldNo: null, newNo: null },
      ],
    },
  ],
};

describe('hunkPatch', () => {
  it('cabecera diff --git, ---/+++ y solo el hunk pedido', () => {
    const p = hunkPatch(diff, 1);
    expect(p.startsWith('diff --git a/f.txt b/f.txt\n')).toBe(true);
    expect(p).toContain('--- a/f.txt\n+++ b/f.txt\n@@ -10,2 +10,3 @@\n');
    expect(p).not.toContain('@@ -1,3');
    expect(p).toBe(HEADER + '@@ -10,2 +10,3 @@\n x\n+y\n z\n\\ No newline at end of file\n');
  });

  it('primer hunk con contexto de función', () => {
    expect(hunkPatch(diff, 0)).toBe(HEADER + '@@ -1,3 +1,3 @@ fn x\n a\n-b\n+B\n c\n');
  });

  it('archivo nuevo', () => {
    const nu: FileDiff = {
      path: 'n.txt', oldPath: null, binary: false, tooLarge: false,
      header: 'diff --git a/n.txt b/n.txt\nnew file mode 100644\nindex 0000000..e69de29\n--- /dev/null\n+++ b/n.txt\n',
      hunks: [{ header: '@@ -0,0 +1,1 @@', oldStart: 0, oldLines: 0, newStart: 1, newLines: 1, lines: [{ kind: '+', text: 'hola', oldNo: null, newNo: 1 }] }],
    };
    const p = hunkPatch(nu, 0);
    expect(p).toContain('new file mode 100644');
    expect(p).toContain('--- /dev/null\n+++ b/n.txt\n@@ -0,0 +1,1 @@\n+hola\n');
  });

  it('índice fuera de rango devuelve vacío', () => {
    expect(hunkPatch(diff, 9)).toBe('');
  });
});

describe('diffStats', () => {
  it('cuenta + y -', () => {
    expect(diffStats(diff)).toEqual({ additions: 2, deletions: 1 });
  });
});
