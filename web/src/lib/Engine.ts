import * as logic from 'logic-mesh'
import { registerBlocks } from './JsBlocks'

export const engine = logic.initEngine()

// Register JS blocks
registerBlocks(engine)

export const blocks = engine.listBlocks()
export const command = engine.engineCommand()

export interface Notification {
	id: string

	changes: { name: string; source: string; value: {} }[]
}

export function startWatch(callback: (notification: Notification) => void) {
	const watchCommand = engine.engineCommand()

	watchCommand.createWatch(callback)
}

// Start the engine
engine.run()
