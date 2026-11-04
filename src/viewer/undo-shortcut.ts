export function isUndoShortcut(
  event: Pick<
    KeyboardEvent,
    'key' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'repeat' | 'target'
  >,
): boolean {
  if (
    event.key.toLowerCase() !== 'z' ||
    !(event.metaKey || event.ctrlKey) ||
    event.altKey ||
    event.shiftKey ||
    event.repeat
  )
    return false;
  const target = event.target;
  return !(
    typeof Element !== 'undefined' &&
    target instanceof Element &&
    target.closest('input, textarea, select, [contenteditable]:not([contenteditable="false"])')
  );
}
