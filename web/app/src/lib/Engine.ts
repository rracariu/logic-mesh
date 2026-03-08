import type { BlockDesc, BlockNotification, BlocksEngine, EngineCommand } from 'logic-mesh';
import { initEngine } from 'logic-mesh';
import { registerBlocks } from './JsBlocks';

let engine: BlocksEngine;
let blocks: BlockDesc[];
let command: EngineCommand;

export function useEngine() {
	if (!engine) {
		engine = initEngine();
		registerBlocks(engine);
		blocks = engine.listBlocks();
		command = engine.engineCommand();
	}

	function startWatch(callback: (notification: BlockNotification) => void) {
		const watchCommand = engine.engineCommand();
		watchCommand.createWatch(callback);
	}

	return { engine, blocks, command, startWatch };
}
