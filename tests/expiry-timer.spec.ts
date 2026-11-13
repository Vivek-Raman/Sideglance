import { test, expect } from 'playwright/test';

test('30-day retention uses a real browser timeout without overflowing into a callback loop', async ({
  page,
}) => {
  await page.route('**/expiry-harness', (route) =>
    route.fulfill({ contentType: 'text/html', body: '<!doctype html><title>Expiry timer</title>' }),
  );
  await page.goto('/expiry-harness');
  const result = await page.evaluate(async () => {
    const { ViewerController, MAX_TIMEOUT_MS } = await import('/src/viewer/controller.ts');
    const delays: number[] = [];
    let fired = 0;
    const controller = new ViewerController(
      {
        now: () => Date.now(),
        reducedMotion: () => true,
        setTimeout: (handler: () => void, delay: number) => {
          delays.push(delay);
          return window.setTimeout(() => {
            fired++;
            handler();
          }, delay);
        },
        clearTimeout: (id: number) => window.clearTimeout(id),
      },
      {
        exit() {},
        move(commit: () => void) {
          commit();
        },
        enter() {},
        async dismiss() {},
        restore() {},
        dispose() {},
      },
    );
    controller.acceptSettings({ autoClearAfterMinutes: 43_200, undoLimit: 3 });
    const at = new Date().toISOString();
    controller.acceptSnapshot({
      revision: 1,
      document: {
        version: 2,
        entries: [
          {
            folder_name: 'timer-test',
            session_id: 'test',
            branch: 'main',
            updated_at: at,
            points: [
              {
                id: 'one',
                text: 'fresh',
                done: false,
                pinned: false,
                link: null,
                submitted_at: at,
              },
            ],
          },
        ],
      },
      health: 'current',
      error: null,
      path: null,
      modified_at: null,
    });
    await new Promise((resolve) => window.setTimeout(resolve, 40));
    controller.dispose();
    return { delays, fired, maximum: MAX_TIMEOUT_MS };
  });
  expect(result.delays).toEqual([result.maximum]);
  expect(result.fired).toBe(0);
});
