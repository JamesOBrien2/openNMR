import { describe, expect, it } from 'vitest';
import { elementPath } from './cursors';

const attrs = (values: Record<string, string>) => (name: string) => values[name] ?? null;

describe('elementPath', () => {
  it('turns each kind of icon element into path data', () => {
    expect(elementPath('path', attrs({ d: 'm21 21-4.34-4.34' }))).toBe('m21 21-4.34-4.34');
    expect(elementPath('line', attrs({ x1: '1', y1: '2', x2: '3', y2: '4' }))).toBe('M1 2L3 4');
    expect(elementPath('circle', attrs({ cx: '11', cy: '11', r: '8' }))).toBe(
      'M3 11a8 8 0 1 0 16 0a8 8 0 1 0 -16 0',
    );
    expect(elementPath('polyline', attrs({ points: '1,2 3,4 5,6' }))).toBe('M1 2L3 4L5 6');
    expect(elementPath('polygon', attrs({ points: '1 2 3 4 5 6' }))).toBe('M1 2L3 4L5 6Z');
    expect(elementPath('rect', attrs({ x: '2', y: '3', width: '4', height: '5' }))).toBe(
      'M2 3h4v5h-4Z',
    );
  });

  it('keeps a rounded rectangle inside its box', () => {
    const d = elementPath('rect', attrs({ x: '0', y: '0', width: '10', height: '4', rx: '9' }));
    // The radius is held to half the shorter side, so the corners meet.
    expect(d.startsWith('M2 0h6a2 2')).toBe(true);
  });

  it('ignores what it cannot draw', () => {
    expect(elementPath('g', attrs({}))).toBe('');
  });
});
