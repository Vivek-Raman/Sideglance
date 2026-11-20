import { test, expect } from 'playwright/test';

test.beforeEach(async ({ page }) => {
  await page.route('**/tests/settings-navigation.html', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: '<!doctype html><html><body><script type="module" src="/tests/lazy-route-fixture.js"></script></body></html>',
    }),
  );
});

test('setup links select Agents on initial load and in an existing settings window', async ({
  page,
}) => {
  await page.goto('/tests/settings-navigation.html#/settings?section=agents');
  const agents = page.getByRole('navigation').getByRole('button', { name: 'Agents', exact: true });
  await expect(agents).toHaveAttribute('aria-current', 'page');
  await page.getByRole('navigation').getByRole('button', { name: 'Board', exact: true }).click();
  await expect(agents).not.toHaveAttribute('aria-current', 'page');
  await page.evaluate(() => {
    window.location.hash = '/settings?section=agents';
  });
  await expect(agents).toHaveAttribute('aria-current', 'page');
  await expect(page.getByLabel('Theme', { exact: true })).toHaveCount(0);
});

test('unknown settings sections fall back to Appearance', async ({ page }) => {
  await page.goto('/tests/settings-navigation.html#/settings?section=unknown');
  await expect(
    page.getByRole('navigation').getByRole('button', { name: 'Appearance', exact: true }),
  ).toHaveAttribute('aria-current', 'page');
  await expect(page.getByLabel('Theme', { exact: true })).toBeVisible();
});
