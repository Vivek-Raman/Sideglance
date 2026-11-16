import { test, expect } from 'playwright/test';
test.beforeEach(async ({ page }) => {
  await page.route('**/tests/updater-lifecycle.html', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: '<!doctype html><html><body><script type="module" src="/tests/updater-lifecycle-fixture.js"></script></body></html>',
    }),
  );
  await page.goto('/tests/updater-lifecycle.html');
  await expect(page.getByRole('button', { name: 'Check for updates' })).toBeEnabled();
});
test('unmount closes an available update exactly once', async ({ page }) => {
  await page.getByRole('button', { name: 'Check for updates' }).click();
  await expect(page.getByRole('button', { name: 'Download and install' })).toBeVisible();
  await page.evaluate(() => window.updaterFixture.remove());
  await expect.poll(() => page.evaluate(() => window.updaterFixture.closed)).toEqual([7]);
});
test('late check results close their resource after unmount', async ({ page }) => {
  await page.evaluate(() => window.updaterFixture.delayCheck());
  await page.getByRole('button', { name: 'Check for updates' }).click();
  await page.evaluate(async () => {
    await window.updaterFixture.remove();
    window.updaterFixture.resolveCheck();
  });
  await expect.poll(() => page.evaluate(() => window.updaterFixture.closed)).toEqual([7]);
});
test('in-flight install keeps its resource until completion after unmount', async ({ page }) => {
  await page.getByRole('button', { name: 'Check for updates' }).click();
  await page.getByRole('button', { name: 'Download and install' }).click();
  await expect(page.getByText('0 KB downloaded')).toBeVisible();
  await page.evaluate(() => window.updaterFixture.remove());
  expect(await page.evaluate(() => window.updaterFixture.closed)).toEqual([]);
  await page.evaluate(() => window.updaterFixture.resolveInstall());
  await expect.poll(() => page.evaluate(() => window.updaterFixture.closed)).toEqual([7]);
});
