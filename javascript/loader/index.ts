/**
 * Main exports for the Auwgent runtime loader
 */

// Core classes
export { Agent } from './IrInterpreter';
export { StreamBuilder } from './StreamBuilder';
export { WorkflowRunner } from './WorkflowRunner';
export { ExpressionEvaluator } from './ExpressionEvaluator';
export { Synthesizer } from './Synthesizer';

// Drivers
export { GoogleDriver } from './drivers/GoogleDriver';
export { OpenAIDriver } from './drivers/OpenAIDriver';
export { DriverRegistry } from './DriverRegistry';

// Types
export type {
    AgentDriver,
    DriverResult,
    StreamChunk,
    SyntheticMessage,
    SyntheticRequest,
    SyntheticToolDef,
    JsonSchema,
    LifecycleHooks,
    ConversationState
} from './types/protocol';

export type {
    AgentIR,
    HelperIR,
    WorkflowIR,
    StepIR,
    Statement,
    Expression
} from './types/ir';

export type { ToolMap, Tool } from './types/tool';

// Error types
export {
    DriverError,
    SchemaValidationError,
    LifecycleError,
    WorkflowError,
    StreamError,
    ConfigurationError
} from './types/errors';

export type { ErrorType } from './types/errors';

// Configuration
export type { RunConfig } from './IrInterpreter';
