import { test, expect } from 'playwright/test';

test('artwork pauses actual SVG playback on blur and reduced motion', async ({ page }) => {
  await page.goto('/tests/artwork-fixture.html');
  await expect(page.locator('svg')).toHaveCount(2);
  await page.evaluate(() => window.dispatchEvent(new Event('focus')));
  await expect
    .poll(() =>
      page
        .locator('svg')
        .first()
        .evaluate((svg: SVGSVGElement) => svg.animationsPaused()),
    )
    .toBe(false);
  await page.evaluate(() => window.dispatchEvent(new Event('blur')));
  const paused = await page
    .locator('svg')
    .first()
    .evaluate(async (svg: SVGSVGElement) => {
      const before = svg.getCurrentTime();
      await new Promise((resolve) => setTimeout(resolve, 80));
      return { paused: svg.animationsPaused(), elapsed: svg.getCurrentTime() - before };
    });
  expect(paused.paused).toBe(true);
  expect(paused.elapsed).toBeLessThan(0.01);
  await page.evaluate(() => window.dispatchEvent(new Event('focus')));
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await expect
    .poll(() =>
      page
        .locator('svg')
        .first()
        .evaluate((svg: SVGSVGElement) => svg.animationsPaused()),
    )
    .toBe(true);
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await expect
    .poll(() =>
      page
        .locator('svg')
        .first()
        .evaluate((svg: SVGSVGElement) => svg.animationsPaused()),
    )
    .toBe(false);
});
