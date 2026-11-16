import { test } from 'node:test';
import assert from 'node:assert/strict';
import { startUpdateSchedule } from '../src/shared/update-schedule.ts';

test('checks at launch and resamples jitter after failures; cancels on disposal', async () => {
  let checks = 0;
  let callback: () => void = () => {};
  const delays: number[] = [];
  let cleared = false;
  const random = [0, 0.999];
  const stop = startUpdateSchedule(
    async () => {
      checks++;
      if (checks === 1) throw new Error('offline');
    },
    {
      setTimeout: (fn, delay) => {
        callback = fn;
        delays.push(delay);
        return 1 as unknown as ReturnType<typeof setTimeout>;
      },
      clearTimeout: () => {
        cleared = true;
      },
      random: () => random.shift()!,
    },
  );
  await Promise.resolve();
  assert.equal(checks, 1);
  assert.deepEqual(delays, [10_801_000]);
  callback();
  await Promise.resolve();
  assert.equal(checks, 2);
  assert.deepEqual(delays, [10_801_000, 10_810_000]);
  stop();
  assert.equal(cleared, true);
});

test('disposal during an in-flight check prevents rescheduling', async () => {
  let finish!: () => void;
  let scheduled = false;
  const stop = startUpdateSchedule(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
    {
      setTimeout: () => {
        scheduled = true;
        return 1 as unknown as ReturnType<typeof setTimeout>;
      },
      clearTimeout: () => {},
      random: () => 0,
    },
  );
  stop();
  finish();
  await Promise.resolve();
  assert.equal(scheduled, false);
});

test('deferred launch leaves the initial check to the user and schedules background checks', async () => {
  let checks = 0;
  let callback: () => void = () => {};
  const delays: number[] = [];
  const stop = startUpdateSchedule(
    async () => {
      checks++;
    },
    {
      setTimeout: (fn, delay) => {
        callback = fn;
        delays.push(delay);
        return 1 as unknown as ReturnType<typeof setTimeout>;
      },
      clearTimeout: () => {},
      random: () => 0,
    },
    { immediate: false },
  );
  assert.equal(checks, 0);
  assert.deepEqual(delays, [10_801_000]);
  callback();
  await Promise.resolve();
  assert.equal(checks, 1);
  assert.equal(delays.length, 2);
  stop();
});
