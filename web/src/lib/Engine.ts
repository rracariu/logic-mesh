import * as logic from 'logic-mesh'

const engine = logic.initEngine()
export const blocks = engine.listBlocks()
export const command = engine.engineCommand()


export interface Notification {
	id: string
	
	changes: { name: string, source: string, value: {} }[]
}

export function startWatch(callback: (notification: Notification) => void) {
	const watchCommand = engine.engineCommand()

	watchCommand.createWatch(callback)
}

// Start the engine
engine.run()