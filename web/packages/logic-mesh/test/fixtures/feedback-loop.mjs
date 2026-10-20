// Runs in a worker thread for FeedbackLoop.test.ts, so that a frozen
// event loop freezes only the worker, which the test can terminate.
//
// Builds a feedback loop of Add blocks — one block fed by its own
// output, or two feeding each other (`workerData.blocks`) — then
// reports the loop's output before and after waiting on a timer: that
// the timer fires while the loop runs, and that the loop does run.
import { parentPort, workerData } from 'node:worker_threads';
import { startEngine } from '../../dist/index.js';

const session = startEngine({ scanInterval: 10 });
const { command } = session;

const first = await command.addBlock('Add');
const second = workerData.blocks === 2 ? await command.addBlock('Add') : first;
await command.writeBlockInput(first, 'in1', 1);
await command.writeBlockInput(second, 'in1', 1);
await command.createLink(first, second, 'out', 'in0');
if (second !== first) {
  await command.createLink(second, first, 'out', 'in0');
}
await command.writeBlockInput(first, 'in0', 1);

const before = (await command.inspectBlock(first)).outputs.out.val;
await new Promise((resolve) => setTimeout(resolve, 100));
const after = (await command.inspectBlock(first)).outputs.out.val;

await session.stop();
parentPort.postMessage({ before, after });
