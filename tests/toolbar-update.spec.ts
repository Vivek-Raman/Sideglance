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

for (const theme of ['light', 'dark']) {
  test(`toolbar hover styles match across controls and update stages in ${theme} theme`, async ({
    page,
  }) => {
    await page.evaluate(
      (theme) => document.documentElement.classList.toggle('dark', theme === 'dark'),
      theme,
    );
    const buttons = page.locator('.document-actions button');
    await expect(buttons).toHaveCount(5);
    const style = async (button: ReturnType<typeof page.locator>) =>
      button.evaluate((element) => {
        const css = getComputedStyle(element);
        return Object.fromEntries(
          [
            'width',
            'height',
            'padding',
            'border',
            'borderRadius',
            'color',
            'backgroundColor',
            'backdropFilter',
            'opacity',
            'cursor',
            'outline',
          ].map((key) => [key, css[key as keyof CSSStyleDeclaration]]),
        );
      });
    const settings = page.getByRole('button', { name: 'Settings', exact: true });
    await settings.hover();
    const hovered = await style(settings);
    for (const button of await buttons.all()) {
      await button.hover();
      expect(await style(button)).toEqual(hovered);
      await button.focus();
      expect(await button.evaluate((el) => getComputedStyle(el).outlineStyle)).toBe('solid');
      await button.blur();
    }
    await expect(buttons.last()).toHaveAccessibleName('Check for updates');
    for (const label of ['Check for updates', 'Download update', 'Install update']) {
      const update = page.getByRole('button', { name: label, exact: true });
      await update.hover();
      expect(await style(update)).toEqual(hovered);
      if (label === 'Install update')
        await page.evaluate(() => window.toolbarFixture.fail('restart'));
      await update.click();
    }
    const restart = page.getByRole('button', { name: 'Restart to finish updating', exact: true });
    await restart.hover();
    expect(await style(restart)).toEqual(hovered);
  });
}

test('disabled text size and busy update controls do not gain hover styles', async ({ page }) => {
  await page.evaluate(() => {
    document.querySelectorAll<HTMLButtonElement>('.document-actions > button').forEach((button) => {
      if (button.getAttribute('aria-label')?.includes('text size')) button.disabled = true;
    });
    window.toolbarFixture.defer('check');
  });
  for (const label of ['Decrease text size', 'Increase text size']) {
    const button = page.getByRole('button', { name: label });
    await button.hover();
    await expect(button).toHaveCSS('border-top-color', 'rgba(0, 0, 0, 0)');
    await expect(button).toHaveCSS('cursor', 'default');
  }
  await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
  const busy = page.getByRole('button', { name: 'Checking for updates', exact: true });
  await expect(busy).toBeDisabled();
  await busy.hover();
  await expect(busy).toHaveCSS('border-top-color', 'rgba(0, 0, 0, 0)');
  await expect(busy).toHaveCSS('cursor', 'default');
});

test('toolbar buttons and icons share a horizontal center line', async ({ page }) => {
  for (const scale of [0.85, 1, 1.15, 1.35]) {
    await page.evaluate((scale) => {
      document.documentElement.style.fontSize = `${14 * scale}px`;
    }, scale);
    const centers = await page.locator('.document-actions button').evaluateAll((buttons) =>
      buttons.map((button) => {
        const box = button.getBoundingClientRect();
        const icon = button.querySelector('svg')!.getBoundingClientRect();
        return { button: box.y + box.height / 2, icon: icon.y + icon.height / 2 };
      }),
    );
    for (const center of centers) {
      expect(Math.abs(center.button - centers[0].button)).toBeLessThan(0.1);
      expect(Math.abs(center.icon - centers[0].icon)).toBeLessThan(0.1);
    }
  }
});
