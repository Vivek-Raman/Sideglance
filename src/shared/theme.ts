import { getCurrentWindow } from '@tauri-apps/api/window';
import { readableAccent } from './accent';
import type { Theme } from './types';

export const systemTheme = window.matchMedia('(prefers-color-scheme: dark)');

let primaryColor = '#c98c5a';

function applyReadableColors(dark: boolean) {
  document.documentElement.style.setProperty('--accent-text', readableAccent(primaryColor, dark));
  document.documentElement.style.setProperty('--focus-ring', readableAccent(primaryColor, dark, 3));
}

export function applyTheme(theme: Theme) {
  const dark = theme === 'dark' || (theme === 'system' && systemTheme.matches);
  document.documentElement.classList.toggle('dark', dark);
  applyReadableColors(dark);
  document.documentElement.style.colorScheme = dark ? 'dark' : 'light';
  void getCurrentWindow()
    .setTheme(theme === 'system' ? null : theme)
    .catch(console.error);
}

export function applyPrimaryColor(color: string) {
  primaryColor = color;
  document.documentElement.style.setProperty('--accent', color);
  applyReadableColors(document.documentElement.classList.contains('dark'));
}
