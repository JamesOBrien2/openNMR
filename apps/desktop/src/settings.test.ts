import { beforeEach, expect, it } from 'vitest';
import { remember, remembered } from './settings';

const store = new Map<string, string>();
beforeEach(() => {
  store.clear();
  globalThis.localStorage = {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => void store.set(k, v),
  } as Storage;
});

it('keeps what was left, and the default for anything missing or of the wrong kind', () => {
  const defaults = { labels: true, noise: 5, font: 'Arial' };
  expect(remembered('k', defaults)).toEqual(defaults);
  remember('k', { labels: false, noise: 'five', extra: 1 });
  expect(remembered('k', defaults)).toEqual({ labels: false, noise: 5, font: 'Arial' });
  store.set('k', 'not json');
  expect(remembered('k', defaults)).toEqual(defaults);
});
