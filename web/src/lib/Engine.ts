import { BlockNotification, initEngine } from 'logic-mesh'
import { registerBlocks } from './JsBlocks'

export type { BlockNotification, BlockDesc, LinkData } from 'logic-mesh'

export const engine = initEngine()

// Register JS blocks
registerBlocks(engine)

export const blocks = engine.listBlocks()
export const command = engine.engineCommand()

export function startWatch(
	callback: (notification: BlockNotification) => void
) {
	const watchCommand = engine.engineCommand()

	watchCommand.createWatch(callback)
}

// Start the engine
engine.run()
