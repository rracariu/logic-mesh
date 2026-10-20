import type {
  BlockDesc,
  BlockNotification,
  EngineCommand,
  EngineSession,
} from 'logic-mesh';
import { createEngineSession } from 'logic-mesh';
import { registerUiConnector } from './UiConnector';
import { widgetBlockDescs } from './Widgets';

let session: EngineSession;
let blocks: BlockDesc[];
let command: EngineCommand;

export function useEngine() {
  if (!session) {
    // The session pre-creates its command handles before the engine
    // can run, which is the only time handles can be created (see the
    // EngineSession docs in the logic-mesh package for the wasm borrow
    // trap the ordering avoids). The engine itself is started later,
    // from the page's onMount, via `start()`. A handle takes
    // overlapping calls, each answered on its own, and the engine
    // handles them in the order they are made.
    session = createEngineSession();
    command = session.command;
    registerUiConnector();
    blocks = [...session.engine.listBlocks(), ...widgetBlockDescs];
  }

  // Opens a watch; a dev-time HMR remount re-running onMount opens
  // another one, which is harmless — every watch keeps delivering.
  function startWatch(callback: (notification: BlockNotification) => void) {
    session.watch(callback).catch((err: unknown) => {
      console.error('Engine: opening a block watch failed:', err);
    });
  }

  // Kicks off the engine's message loop, un-awaited — the session owns
  // the ordering rules and marks itself started.
  function start() {
    session.start();
  }

  // Resolves once the engine has reset, so a request issued afterwards
  // sees it empty.
  function reset(): Promise<void> {
    return session.reset();
  }

  return {
    start,
    reset,
    blocks,
    command,
    connectorCommand: session.connectorCommand,
    startWatch,
  };
}
