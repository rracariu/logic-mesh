// A feedback loop — a block fed by its own output, or blocks feeding
// each other — cycles without ever waiting on a timer. The engine's
// tasks are woken through microtasks, so without a yield to the host
// such a loop kept every timer, I/O callback and render from running:
// the page or process froze for good.
//
// Each case runs in a worker thread (test/fixtures/feedback-loop.mjs),
// against the built package in `dist/`: a frozen worker cannot freeze
// the test runner, which terminates it after a deadline instead.
import { Worker } from 'node:worker_threads';
import { describe, expect, it } from 'vitest';

/** What the fixture reports: the loop's output before and after a
 * 100 ms timer. */
interface LoopReport {
  before: number;
  after: number;
}

/** Runs the fixture with a loop of `blocks` Add blocks, rejecting if
 * it reports nothing within `deadlineMs`. */
function runLoop(blocks: 1 | 2, deadlineMs = 3000): Promise<LoopReport> {
  const worker = new Worker(
    new URL('./fixtures/feedback-loop.mjs', import.meta.url),
    { workerData: { blocks }, stderr: true },
  );
  return new Promise<LoopReport>((resolve, reject) => {
    const deadline = setTimeout(() => {
      reject(
        new Error(
          `no report within ${deadlineMs} ms: the loop froze the event loop`,
        ),
      );
    }, deadlineMs);
    worker.once('message', (report: LoopReport) => {
      clearTimeout(deadline);
      resolve(report);
    });
    worker.once('error', (err) => {
      clearTimeout(deadline);
      reject(err);
    });
  }).finally(() => worker.terminate());
}

describe('a feedback loop', () => {
  it('of one block fed by its own output leaves timers running', async () => {
    const { before, after } = await runLoop(1);
    // The timer fired, and the loop kept cycling meanwhile.
    expect(after).toBeGreaterThan(before);
  });

  it('of two blocks feeding each other leaves timers running', async () => {
    const { before, after } = await runLoop(2);
    expect(after).toBeGreaterThan(before);
  });
});
