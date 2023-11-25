import {
	BlockDesc,
	BlockNotification,
	BlocksEngine,
	EngineCommand,
	initEngine,
} from 'logic-mesh'
import { registerBlocks } from './JsBlocks'

let engine: BlocksEngine
let blocks: BlockDesc[]
let command: EngineCommand

export function useEngine() {
	if (!engine) {
		engine = initEngine()
		// Register JS blocks
		registerBlocks(engine)
		blocks = engine.listBlocks()
		command = engine.engineCommand()
	}

	function startWatch(callback: (notification: BlockNotification) => void) {
		const watchCommand = engine.engineCommand()

		watchCommand.createWatch(callback)
	}

	return {
		engine,
		blocks,
		command,
		startWatch,
	}
}
