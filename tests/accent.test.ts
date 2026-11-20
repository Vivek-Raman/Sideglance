import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readableAccent } from '../src/shared/accent.ts';

function luminance(hex: string) {
  const rgb = hex
    .slice(1)
    .match(/../g)!
    .map((v) => parseInt(v, 16) / 255);
  return rgb.reduce(
    (sum, value, i) =>
      sum +
      (value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4) *
        [0.2126, 0.7152, 0.0722][i],
    0,
  );
}
function contrast(a: string, b: string) {
  const values = [luminance(a), luminance(b)].sort((x, y) => x - y);
  return (values[1] + 0.05) / (values[0] + 0.05);
}
test('accent text and focus colors remain readable for varied user colors on both themes', () => {
  for (const color of [
    '#000000',
    '#ffffff',
    '#c98c5a',
    '#ff0000',
    '#00ff00',
    '#0000ff',
    '#808080',
    '#f7f7f8',
    '#202124',
  ]) {
    for (const [dark, background] of [
      [false, '#f7f7f8'],
      [true, '#202124'],
    ] as const) {
      assert.ok(
        contrast(readableAccent(color, dark), background) >= 4.5,
        `${color} text on ${background}`,
      );
      assert.ok(
        contrast(readableAccent(color, dark, 3), background) >= 3,
        `${color} focus on ${background}`,
      );
    }
  }
});
