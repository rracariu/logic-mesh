/**
 * The stable, machine-readable code of a failed engine command — the
 * name of the `ErrorCode` the Rust engine reported. A code never
 * changes meaning; new failures get new codes.
 *
 * The union is closed for each package version: the engine that
 * rejects is the wasm module shipped in this package, and its test
 * suite checks this list against its own codes, so it names every code
 * a command can reject with. A later version may add codes, so keep a
 * `default` branch when switching on one.
 */
export type LogicMeshErrorCode =
  // Block registry
  | 'BlockNotFound'
  | 'BlockNotRegistered'
  | 'AmbiguousBlockName'
  | 'BlockAlreadyRegistered'
  | 'TooManyInputs'
  // Engine
  | 'BlockInstanceNotFound'
  | 'DuplicateBlockId'
  | 'NoRuntime'
  | 'InvalidBlockUuid'
  | 'PinNotFound'
  | 'InputNotFound'
  | 'OutputNotFound'
  | 'LinkBlockNotInProgram'
  | 'DuplicateLink'
  | 'DuplicateLinkId'
  | 'InvalidLinkUuid'
  | 'UnsupportedProgramVersion'
  | 'BlockVersionMismatch'
  | 'BlockTaskGone'
  | 'BlockDroppedReply'
  | 'ConnectorAlreadyManaged'
  | 'ConnectorNotManaged'
  | 'ConnectorStartFailed'
  | 'ConnectorStartTimeout'
  | 'EngineGone'
  | 'UnexpectedResponse'
  // Links
  | 'AlreadyConnected'
  | 'SelfLink'
  | 'NotConnected'
  // Values
  | 'KindConversion'
  | 'UnexpectedValue'
  | 'ValueConversion'
  | 'UnitConversion'
  | 'ZincDecode'
  | 'ZincEncode'
  | 'ParseBool'
  // External blocks and JS connectors
  | 'ExternalUnsupported'
  | 'ExternalUnsupportedMultiThreaded'
  | 'ExternalLibraryNotFound'
  | 'ExternalFactoryCallFailed'
  | 'ExternalFactoryReturnedNonFunction'
  | 'ConnectorPropertyUnreadable'
  | 'ConnectorMethodMissing'
  | 'ConnectorMethodNotFunction'
  // Connectors
  | 'ConnectorNotFound'
  | 'ConnectorAlreadyRegistered'
  | 'ConnectorSubscribeFailed'
  | 'ConnectorPublishFailed'
  | 'ConnectorRequestFailed'
  | 'ConnectorTimeout'
  | 'ConnectorTransport'
  // Command arguments and results
  | 'InvalidProgram'
  | 'InvalidArgument'
  | 'Serialization';

/**
 * The `Error` an `EngineCommand` promise rejects with: `name` is
 * `'LogicMeshError'`, `code` says what failed, and `message` describes
 * it, including the underlying causes.
 *
 * ```ts
 * try {
 *   await command.loadProgram(program);
 * } catch (err) {
 *   if (isLogicMeshError(err) && err.code === 'UnsupportedProgramVersion') {
 *     // Saved by a newer version of the editor.
 *   }
 * }
 * ```
 */
export interface LogicMeshError extends Error {
  name: 'LogicMeshError';
  code: LogicMeshErrorCode;
}

/**
 * Whether `err` is a {@link LogicMeshError}: an engine command's
 * rejection.
 */
export function isLogicMeshError(err: unknown): err is LogicMeshError {
  return (
    err instanceof Error &&
    err.name === 'LogicMeshError' &&
    typeof (err as { code?: unknown }).code === 'string'
  );
}
