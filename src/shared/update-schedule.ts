/** Check every three hours with jitter; optionally defer the first check. */
export function startUpdateSchedule(
  check: () => Promise<void>,
  timers = {
    setTimeout: (callback: () => void, delay: number) => globalThis.setTimeout(callback, delay),
    clearTimeout: (id: ReturnType<typeof setTimeout>) => globalThis.clearTimeout(id),
    random: Math.random,
  },
  options: { immediate?: boolean } = {},
) {
  let stopped = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  function schedule() {
    const jitter = 1 + Math.floor(timers.random() * 10);
    timer = timers.setTimeout(() => void run(), 3 * 60 * 60 * 1000 + jitter * 1000);
  }
  async function run() {
    try {
      await check();
    } catch {
      // Background failures should not interrupt the viewer. Retry next interval.
    } finally {
      if (!stopped) schedule();
    }
  }
  if (options.immediate === false) schedule();
  else void run();
  return () => {
    stopped = true;
    if (timer !== undefined) timers.clearTimeout(timer);
  };
}
