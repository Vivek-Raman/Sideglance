/** Check immediately, then after three hours plus fresh 1–10 second jitter. */
export function startUpdateSchedule(
  check: () => Promise<void>,
  timers = {
    setTimeout: (callback: () => void, delay: number) => globalThis.setTimeout(callback, delay),
    clearTimeout: (id: ReturnType<typeof setTimeout>) => globalThis.clearTimeout(id),
    random: Math.random,
  },
) {
  let stopped = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  async function run() {
    try {
      await check();
    } catch {
      // Background failures should not interrupt the viewer. Retry next interval.
    } finally {
      if (!stopped) {
        const jitter = 1 + Math.floor(timers.random() * 10);
        timer = timers.setTimeout(() => void run(), 3 * 60 * 60 * 1000 + jitter * 1000);
      }
    }
  }
  void run();
  return () => {
    stopped = true;
    if (timer !== undefined) timers.clearTimeout(timer);
  };
}
