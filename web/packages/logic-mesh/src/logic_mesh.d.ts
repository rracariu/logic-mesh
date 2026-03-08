/* tslint:disable */
/* eslint-disable */

/**
 * Controls the execution or the blocks.
 * Loads programs and enables inspection and debugging
 * of the blocks and their inputs and outputs.
 */
export class BlocksEngine {
    free(): void;
    [Symbol.dispose](): void;
    engineCommand(): EngineCommand;
    /**
     * Lists all available blocks
     */
    listBlocks(): Array<any>;
    /**
     * Create a new instance of an engine
     */
    constructor(sleep_duration?: bigint | null);
    /**
     * Register a new JS block in the registry
     * The block is described by a JsBlockDesc object
     *
     * # Arguments
     * * `desc` - The description of the block
     * * `func` - Optional function that implements the block
     * 		  logic. If not provided, the block would do nothing.
     *
     * # Returns
     * The name of the block
     */
    registerBlock(desc: any, func?: Function | null): string;
    /**
     * Runs the engine asynchronously
     * After this is called, the engine instance can't be used directly
     * Instead use the command object to communicate with the engine.
     */
    run(): Promise<void>;
}

/**
 * Commands a running instance of a Block Engine.
 */
export class EngineCommand {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Adds a block instance to the engine
     * to be immediately scheduled for execution
     */
    addBlock(block_name: string, block_uuid?: string | null, lib?: string | null): Promise<string>;
    /**
     * Creates a link between two blocks
     *
     * # Arguments
     * * `source_block_uuid` - The UUID of the source block
     * * `target_block_uuid` - The UUID of the target block
     * * `source_block_pin_name` - The name of the output pin of the source block
     * * `target_block_input_name` - The name of the input pin of the target block
     *
     * # Returns
     * A `LinkData` object with the following properties:
     * * `source_block_uuid` - The UUID of the source block
     * * `target_block_uuid` - The UUID of the target block
     * * `source_block_pin_name` - The name of the output pin of the source block
     * * `target_block_input_name` - The name of the input pin of the target block
     */
    createLink(source_block_uuid: string, target_block_uuid: string, source_block_pin_name: string, target_block_pin_name: string): Promise<any>;
    /**
     * Creates a watch on block changes
     */
    createWatch(callback: Function): Promise<void>;
    /**
     * Evaluates a block by name
     * This will create a block instance and execute it.
     *
     * # Arguments
     * * `block_name` - The name of the block to evaluate
     * * `inputs` - The input values to the block
     * * `lib` - Optional, the library to load the block from, defaults to "core"
     *
     * # Returns
     * A list of values representing the outputs of the block
     */
    evalBlock(block_name: string, inputs: any[], lib?: string | null): Promise<any>;
    /**
     * Get the current running engine program.
     * The program contains the scheduled blocks, their properties, and their links.
     */
    getProgram(): Promise<any>;
    /**
     * Inspects the current state of a block
     */
    inspectBlock(block_uuid: string): Promise<any>;
    /**
     * Pauses the execution of the engine
     * If the engine is already paused, this does nothing
     */
    pauseExecution(): Promise<void>;
    /**
     * Removes a block instance from the engine
     * to be immediately unscheduled for execution
     * and removed from the engine together with all its links.
     *
     * # Arguments
     * * `block_uuid` - The UUID of the block to be removed
     *
     * # Returns
     * The UUID of the removed block
     */
    removeBlock(block_uuid: string): Promise<string>;
    /**
     * Removes a link between two blocks
     *
     * # Arguments
     * * `link_uuid` - The UUID of the link to be removed
     *
     * # Returns
     * True if the link was removed, false otherwise
     */
    removeLink(link_uuid: string): Promise<boolean>;
    /**
     * Resets the engine state, clears all blocks and links
     */
    resetEngine(): Promise<void>;
    /**
     * Resumes the execution of the engine
     * If the engine is not paused, this does nothing
     */
    resumeExecution(): Promise<void>;
    /**
     * Stop the engine's execution
     */
    stopEngine(): Promise<void>;
    /**
     * Writes the given block input with a value
     *
     * # Arguments
     * * `block_uuid` - The UUID of the block to write to
     * * `input_name` - The name of the input to write to
     * * `value` - The value to write
     *
     * # Returns
     * The previous value of the input
     */
    writeBlockInput(block_uuid: string, input_name: string, value: any): Promise<any>;
    /**
     * Writes the given block out with a value
     *
     * # Arguments
     * * `block_uuid` - The UUID of the block to write to
     * * `output_name` - The name of the output to write to
     * * `value` - The value to write
     *
     * # Returns
     * The block data
     */
    writeBlockOutput(block_uuid: string, output_name: string, value: any): Promise<any>;
}

export function initEngine(sleep_duration?: number | null): BlocksEngine;

export function init_panic_hook(): void;

export function start(): void;
