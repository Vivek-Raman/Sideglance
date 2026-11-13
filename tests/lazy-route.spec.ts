import { test, expect } from 'playwright/test';

test.beforeEach(async ({ page }) => {
  await page.route('**/tests/lazy-route.html', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: '<!doctype html><html><body><script type="module" src="/tests/lazy-route-fixture.js"></script></body></html>',
    }),
  );
});

test('loads settings only after navigating to its route', async ({ page }) => {
  const settingsRequests: string[] = [];
  page.on('request', (request) => {
    if (request.url().includes('/src/settings/Settings.svelte'))
      settingsRequests.push(request.url());
  });
  await page.goto('/tests/lazy-route.html');
  await expect(page.getByRole('region', { name: 'Project status' })).toBeVisible();
  expect(settingsRequests).toHaveLength(0);
  await page.evaluate(() => {
    window.location.hash = '/settings';
  });
  await expect(page.getByRole('heading', { name: 'Appearance', exact: true })).toBeVisible();
  expect(settingsRequests.length).toBeGreaterThan(0);
});

test('failed settings import offers reload and recovers', async ({ page }) => {
  let fail = true;
  await page.route('**/src/settings/Settings.svelte', (route) => {
    if (fail) {
      fail = false;
      return route.abort();
    }
    return route.continue();
  });
  await page.goto('/tests/lazy-route.html#/settings');
  await expect(page.getByRole('alert')).toHaveText('Settings could not be loaded.');
  await page.getByRole('button', { name: 'Reload settings' }).click();
  await expect(page.getByRole('heading', { name: 'Appearance', exact: true })).toBeVisible();
});
