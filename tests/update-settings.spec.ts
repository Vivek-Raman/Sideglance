import { test, expect } from 'playwright/test';

test.beforeEach(async ({ page }) => {
  await page.route('**/tests/update-settings.html', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: '<html><body style="--border: #ccc; --muted: #666; --text: #111; --accent: blue"></body></html>',
    }),
  );
  await page.goto('/tests/update-settings.html');
});

test('renders release note Markdown and removes unsafe HTML', async ({ page }) => {
  await page.evaluate(async () => {
    // @ts-expect-error Vite serves the source module in the browser.
    const { renderReleaseNotes } = await import('/src/settings/release-notes.ts');
    document.body.innerHTML = renderReleaseNotes(
      '## Changes\n\n- **Fixed** updates\n- Use `settings`\n\n[Release](https://example.com/release)\n\n<script>alert(1)</script><img src="x" onerror="alert(1)"><a href="javascript:alert(1)" onclick="alert(1)">unsafe</a>',
    );
  });
  await expect(page.getByRole('heading', { name: 'Changes' })).toBeVisible();
  await expect(page.locator('li')).toHaveCount(2);
  await expect(page.locator('strong')).toHaveText('Fixed');
  await expect(page.locator('code')).toHaveText('settings');
  await expect(page.getByRole('link', { name: 'Release' })).toHaveAttribute(
    'href',
    'https://example.com/release',
  );
  await expect(page.locator('script, img, [onclick], [href^="javascript:"]')).toHaveCount(0);
});

test('updates owns its card and button styles', async ({ page }) => {
  await page.addScriptTag({ type: 'module', url: '/tests/update-settings-fixture.js' });
  const card = page.locator('.settings-group');
  await expect(card).toHaveCSS('display', 'grid');
  await expect(card).toHaveCSS('border-top-width', '1px');
  await expect(card).toHaveCSS('padding-top', '17.6px');
  await expect(page.getByRole('button', { name: 'Check for updates' })).toHaveCSS(
    'font-weight',
    '600',
  );
});
