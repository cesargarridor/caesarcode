import { describe, expect, it } from 'vitest';
import { filterRecent, timeAgo } from './RecentList';
import type { RecentProject } from '../../lib/types';

const p = (name: string, path: string): RecentProject => ({ name, path, lastOpened: 0, kind: null, pinned: false, exists: true });

describe('RecentList helpers', () => {
  it('filtra por nombre o ruta sin distinguir mayúsculas', () => {
    const items = [p('Shop', '/home/u/shop'), p('blog', '/work/Sites/blog')];
    expect(filterRecent(items, '')).toHaveLength(2);
    expect(filterRecent(items, 'SHOP').map((x) => x.name)).toEqual(['Shop']);
    expect(filterRecent(items, 'sites').map((x) => x.name)).toEqual(['blog']);
    expect(filterRecent(items, 'nada')).toEqual([]);
  });

  it('tiempo relativo', () => {
    const now = 10_000_000_000;
    expect(timeAgo(now - 5_000, now)).toBe('ahora');
    expect(timeAgo(now - 5 * 60_000, now)).toBe('hace 5 min');
    expect(timeAgo(now - 3 * 3_600_000, now)).toBe('hace 3 h');
    expect(timeAgo(now - 2 * 86_400_000, now)).toBe('hace 2 d');
  });
});
