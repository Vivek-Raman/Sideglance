import { test, expect } from 'playwright/test';

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.route('**/tests/toolbar-update.html', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: '<!doctype html><html><body><script type="module" src="/tests/toolbar-update-fixture.js"></script></body></html>',
    }),
  );
  await page.goto('/tests/toolbar-update.html');
});

test('check, download, and install run separately and ready actions survive blur', async ({
  page,
}) => {
  const check = page.getByRole('button', { name: 'Check for updates', exact: true });
  await expect(check.locator('svg')).toBeVisible();
  await page.evaluate(() => window.dispatchEvent(new Event('blur')));
  await expect(page.locator('.update-control')).toHaveCSS('opacity', '0');
  await page.evaluate(() => window.dispatchEvent(new Event('focus')));
  await check.click();
  const download = page.getByRole('button', { name: 'Download update', exact: true });
  await expect(download).toBeEnabled();
  expect(
    await page.evaluate(
      () => window.toolbarFixture.calls.filter((x) => x.name === 'plugin:updater|download').length,
    ),
  ).toBe(0);
  await page.evaluate(() => window.dispatchEvent(new Event('blur')));
  await expect(page.locator('.update-control')).toHaveCSS('opacity', '1');
  await expect(page.getByRole('button', { name: 'Settings', exact: true })).toHaveCSS(
    'opacity',
    '0',
  );
  await download.click();
  const install = page.getByRole('button', { name: 'Install update', exact: true });
  await expect(install).toBeEnabled();
  await page.evaluate(() => window.dispatchEvent(new Event('blur')));
  await expect(page.locator('.update-control')).toHaveCSS('opacity', '1');
  expect(
    await page.evaluate(
      () => window.toolbarFixture.calls.filter((x) => x.name === 'plugin:updater|install').length,
    ),
  ).toBe(0);
  await install.click();
  await expect
    .poll(() =>
      page.evaluate(() => window.toolbarFixture.calls.some((x) => x.name === 'restart_app')),
    )
    .toBe(true);
  const calls = await page.evaluate(() => window.toolbarFixture.calls);
  expect(calls.filter((x) => x.name === 'plugin:updater|install')).toEqual([
    { name: 'plugin:updater|install', updateRid: 11, bytesRid: 22 },
  ]);
  expect(calls.some((x) => x.name === 'plugin:updater|download_and_install')).toBe(false);
});

test('busy operations prevent duplicates and report download progress', async ({ page }) => {
  await page.evaluate(() => window.toolbarFixture.defer('check'));
  await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
  await expect(
    page.getByRole('button', { name: 'Checking for updates', exact: true }),
  ).toBeDisabled();
  await page.evaluate(() => window.toolbarFixture.resolve('check'));
  await page.evaluate(() => window.toolbarFixture.defer('download'));
  await page.getByRole('button', { name: 'Download update', exact: true }).click();
  const downloading = page.getByRole('button', { name: 'Downloading update', exact: true });
  await expect(downloading).toBeDisabled();
  await expect(downloading).toHaveAttribute('title', /45%/);
  await page.evaluate(() => window.dispatchEvent(new Event('blur')));
  await expect(page.locator('.update-control')).toHaveCSS('opacity', '1');
  await page.evaluate(() => window.toolbarFixture.resolve('download'));
  await expect(page.getByRole('button', { name: 'Install update', exact: true })).toBeEnabled();
});

test('each failure retains the right retry action; restart retry never reinstalls', async ({
  page,
}) => {
  await page.evaluate(() => window.toolbarFixture.fail('check'));
  await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
  await expect(
    page.getByRole('button', { name: 'Check for updates', exact: true }),
  ).toHaveAttribute('title', /check unavailable/);
  await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
  await page.evaluate(() => window.toolbarFixture.fail('download'));
  await page.getByRole('button', { name: 'Download update', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Download update', exact: true })).toHaveAttribute(
    'title',
    /download unavailable/,
  );
  await page.getByRole('button', { name: 'Download update', exact: true }).click();
  await page.evaluate(() => window.toolbarFixture.fail('install'));
  await page.getByRole('button', { name: 'Install update', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Install update', exact: true })).toHaveAttribute(
    'title',
    /install unavailable/,
  );
  await page.evaluate(() => window.toolbarFixture.fail('restart'));
  await page.getByRole('button', { name: 'Install update', exact: true }).click();
  const restart = page.getByRole('button', { name: 'Restart to finish updating', exact: true });
  await expect(restart).toBeEnabled();
  await restart.click();
  expect(
    await page.evaluate(
      () => window.toolbarFixture.calls.filter((x) => x.name === 'plugin:updater|install').length,
    ),
  ).toBe(2);
});

test('no update keeps the check action available with feedback', async ({ page }) => {
  await page.evaluate(() => window.toolbarFixture.noUpdate());
  await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
  await expect(
    page.getByRole('button', { name: 'Check for updates', exact: true }),
  ).toHaveAttribute('title', /up to date/);
});

test('unmount releases downloaded bytes and updater resources', async ({ page }) => {
  await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
  await page.getByRole('button', { name: 'Download update', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Install update', exact: true })).toBeEnabled();
  await page.evaluate(() => window.toolbarFixture.remove());
  await expect
    .poll(() =>
      page.evaluate(() =>
        window.toolbarFixture.calls
          .filter((x) => x.name === 'plugin:resources|close')
          .map((x) => x.rid),
      ),
    )
    .toEqual([22, 11]);
});

test('late check and download completions release resources after unmount', async ({ page }) => {
  await page.evaluate(() => window.toolbarFixture.defer('check'));
  await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
  await page.evaluate(async () => {
    await window.toolbarFixture.remove();
    window.toolbarFixture.resolve('check');
  });
  await expect
    .poll(() =>
      page.evaluate(() =>
        window.toolbarFixture.calls
          .filter((x) => x.name === 'plugin:resources|close')
          .map((x) => x.rid),
      ),
    )
    .toEqual([11]);
  await page.reload();
  await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
  await page.evaluate(() => window.toolbarFixture.defer('download'));
  await page.getByRole('button', { name: 'Download update', exact: true }).click();
  await page.evaluate(async () => {
    await window.toolbarFixture.remove();
    window.toolbarFixture.resolve('download');
  });
  await expect
    .poll(() =>
      page.evaluate(() =>
        window.toolbarFixture.calls
          .filter((x) => x.name === 'plugin:resources|close')
          .map((x) => x.rid),
      ),
    )
    .toEqual([22, 11]);
});
