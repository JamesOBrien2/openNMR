/** Settings kept between sessions. Each is read over its default, so a setting
 * added later, or a stored value of the wrong kind, falls back to the default
 * rather than breaking the start: what is kept is whatever the user left, and
 * nothing depends on it being there. */
export function remembered<T extends object>(key: string, defaults: T): T {
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(key) ?? 'null');
    if (typeof saved !== 'object' || saved === null) return { ...defaults };
    const kept = Object.entries(defaults).map(([name, value]) => {
      const stored = (saved as Record<string, unknown>)[name];
      return [name, typeof stored === typeof value ? stored : value];
    });
    return Object.fromEntries(kept) as T;
  } catch {
    return { ...defaults };
  }
}

export function remember(key: string, value: object) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Storage full or switched off: the setting lasts for this session only.
  }
}
