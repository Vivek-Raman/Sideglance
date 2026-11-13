import { test, expect } from 'playwright/test';

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.route('**/tests/health.html*', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: '<!doctype html><html><body><script type="module" src="/tests/viewer-health-fixture.js"></script></body></html>',
    }),
  );
});
test('empty, healthy, stale and paused updates remain understandable', async ({ page }) => {
  await page.goto('/tests/health.html');
  await expect(page.getByText('Waiting for updates', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Set up agent integration' })).toBeVisible();
  await page.evaluate(() => window.fixture.set('current', window.fixture.entries()));
  await expect(page.getByText('Point 1', { exact: true })).toBeVisible();
  await expect(page.locator('.session-label').filter({ hasText: 'Session 1' })).toBeVisible();
  await page.evaluate(() => {
    const node = document.querySelector('.point-text');
    const range = document.createRange();
    range.selectNodeContents(node);
    const selection = window.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
    document.dispatchEvent(new Event('selectionchange'));
  });
  await expect(page.getByRole('button', { name: 'Resume updates' })).toBeVisible();
  await page.evaluate(() => window.fixture.set('stale', []));
  await expect(page.getByText('Updates are stale', { exact: true })).toBeVisible();
  await expect(page.getByText('Point 1', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Resume updates' }).click();
  await expect(page.getByText('Point 1', { exact: true })).toHaveCount(0);
});
test('collapsed groups survive reload and archive restoration is explicit', async ({ page }) => {
  await page.goto('/tests/health.html');
  await expect(page.getByRole('button', { name: 'Set up agent integration' })).toBeVisible();
  await page.evaluate(() => window.fixture.set('current', window.fixture.entries()));
  await page.getByRole('button', { name: 'Collapse project', exact: true }).focus();
  await page.getByRole('button', { name: 'Collapse project', exact: true }).press('Enter');
  await expect(
    page.locator('.motion-row').filter({ has: page.getByText('Point 1', { exact: true }) }),
  ).toHaveAttribute('aria-hidden', 'true');
  await page.reload();
  await expect(page.getByRole('button', { name: 'Set up agent integration' })).toBeVisible();
  await page.evaluate(() => {
    window.fixture.set('current', window.fixture.entries());
    window.fixture.removed();
  });
  const expand = page.getByRole('button', { name: 'Expand project', exact: true });
  await expect(expand).toBeVisible();
  await expand.hover();
  await expect(expand).toHaveCSS('opacity', '1');
  await expand.click();
  await expect(page.getByRole('button', { name: 'Collapse project', exact: true })).toBeVisible();
  await page.getByText('Recently removed', { exact: true }).click();
  await page.getByRole('button', { name: 'Restore and pin Point 1' }).click();
  await expect(page.getByText('No recently removed items.')).toBeVisible();
});
test('enabling deletion requires a preview and explicit apply', async ({ page }) => {
  await page.goto('/tests/health.html?mode=retention');
  await page.getByRole('checkbox', { name: 'Time-based deletion' }).check();
  await expect(page.getByRole('button', { name: 'Review removal' })).toBeVisible();
  expect(
    await page.evaluate(() =>
      window.fixture.calls.some((x) => x.name === 'set_auto_clear_after_minutes'),
    ),
  ).toBe(false);
  await page.getByRole('button', { name: 'Review removal' }).click();
  await expect(page.getByRole('status')).toContainText('About 2 items would');
  await page.getByRole('button', { name: 'Apply deletion rule' }).click();
  await expect
    .poll(() =>
      page.evaluate(
        () => window.fixture.calls.filter((x) => x.name === 'set_auto_clear_after_minutes').length,
      ),
    )
    .toBe(1);
});
test('configured integrations offer read-only connection testing and repair', async ({ page }) => {
  await page.goto('/tests/health.html?mode=integrations');
  await page.getByRole('button', { name: 'Test connection' }).click();
  await expect(page.getByRole('status')).toContainText('Connected:');
  await expect(page.getByRole('button', { name: 'Repair MCP' })).toBeVisible();
});
