import test from 'node:test';
import assert from 'node:assert/strict';
import { isUndoShortcut } from '../src/viewer/undo-shortcut.ts';

test('undo accepts Cmd+Z and Ctrl+Z, excluding redo, repeats and unrelated keys', () => {
  const event = {
    key: 'z',
    metaKey: true,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    repeat: false,
    target: null,
  };
  assert.equal(isUndoShortcut(event), true);
  assert.equal(isUndoShortcut({ ...event, metaKey: false, ctrlKey: true }), true);
  for (const change of [
    { metaKey: false },
    { shiftKey: true },
    { altKey: true },
    { repeat: true },
    { key: 'x' },
  ]) {
    assert.equal(isUndoShortcut({ ...event, ...change }), false);
  }
});
