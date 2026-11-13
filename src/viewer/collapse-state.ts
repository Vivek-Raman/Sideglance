const storageKey = 'sideglance.collapsed.v1';

export function loadCollapsed(): Set<string> {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(storageKey) ?? 'null');
    if (!Array.isArray(value)) return new Set();
    return new Set(
      value
        .slice(0, 500)
        .filter((key): key is string => typeof key === 'string' && key.length <= 2048),
    );
  } catch {
    return new Set();
  }
}

export function saveCollapsed(value: ReadonlySet<string>) {
  try {
    localStorage.setItem(storageKey, JSON.stringify([...value].slice(0, 500)));
  } catch {
    /* A disabled or full storage area must not prevent using the viewer. */
  }
}
