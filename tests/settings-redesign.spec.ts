import { test, expect } from 'playwright/test';

test.beforeEach(async ({ page }) => {
  await page.route('**/tests/settings-redesign.html*', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: '<!doctype html><html><body><script type="module" src="/tests/settings-redesign-fixture.js"></script></body></html>',
    }),
  );
});

test('panels follow navigation and external hash changes', async ({ page }) => {
  await page.goto('/tests/settings-redesign.html#/settings');
  await expect(page.getByLabel('Theme')).toBeVisible();
  await page.getByRole('button', { name: 'Board', exact: true }).click();
  await expect(page.getByLabel('Group status items by')).toBeVisible();
  await expect(page.getByLabel('Theme')).toHaveCount(0);
  await page.evaluate(() => {
    location.hash = '/settings?section=agents';
  });
  await expect(page.getByText('Codex', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'App', exact: true }).click();
  await page.getByText('Advanced', { exact: true }).click();
  await expect(page.getByLabel('Undo history size')).toBeVisible();
});

test('setup installs skill, configures MCP and tests connection', async ({ page }) => {
  await page.goto('/tests/settings-redesign.html#/settings?section=agents');
  const row = page.locator('.agent-row').filter({ hasText: 'Codex' });
  await row.getByRole('button', { name: 'Set up' }).click();
  await expect(row.getByText('Configured', { exact: true })).toBeVisible();
  await expect(row.getByText(/Connection test passed/)).toBeVisible();
  expect(
    await page.evaluate(() =>
      (window as unknown as { calls: string[] }).calls.filter((name) =>
        ['install_skill_for_target', 'install_mcp_for_harness', 'test_mcp_connection'].includes(
          name,
        ),
      ),
    ),
  ).toEqual(['install_skill_for_target', 'install_mcp_for_harness', 'test_mcp_connection']);
  const gemini = page.locator('.agent-row').filter({ hasText: 'Gemini' });
  await gemini.getByRole('button', { name: 'Set up' }).click();
  await expect(
    gemini.getByRole('status').filter({ hasText: 'MCP setup is not supported' }),
  ).toBeVisible();
});

test('failed connection keeps a fix action available', async ({ page }) => {
  await page.goto('/tests/settings-redesign.html#/settings?section=agents');
  await page.evaluate(() => {
    (window as unknown as { failConnection: boolean }).failConnection = true;
  });
  const row = page.locator('.agent-row').filter({ hasText: 'Codex' });
  await row.getByRole('button', { name: 'Set up' }).click();
  await expect(row.getByRole('button', { name: 'Fix setup' })).toBeVisible();
  await expect(row.getByText(/Connection needs attention/)).toBeVisible();
});

test('modified skills require explicit replacement', async ({ page }) => {
  await page.goto('/tests/settings-redesign.html?modified=1#/settings?section=agents');
  const row = page.locator('.agent-row').filter({ hasText: 'Codex' });
  await row.getByRole('button', { name: 'Review skill' }).click();
  await expect(page.getByText(/Replacing it will discard/)).toBeVisible();
  expect(
    await page.evaluate(() => (window as unknown as { installArgs: unknown[] }).installArgs),
  ).toEqual([]);
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Replace modified skill' })).toHaveCount(0);
  await row.getByRole('button', { name: 'Review skill' }).click();
  await page.getByRole('button', { name: 'Replace modified skill' }).click();
  await expect(row.getByText('Configured', { exact: true })).toBeVisible();
  expect(
    await page.evaluate(() => (window as unknown as { installArgs: unknown[] }).installArgs),
  ).toEqual([{ target: 'codex', replaceModified: true }]);
});
