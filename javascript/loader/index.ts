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
    ModelUsage,
    ThinkingBlock,
    ToolArgs,
    ToolResult,
    ToolCall,
    MiddlewareContext,
    AgentMiddleware
} from './types/protocol';

export type {
    AgentIR,
    HelperIR,
    Workflow,
    // StepIR, invalid
    Statement,
    Expression
} from './types/ir';

export type { ToolMap, ToolImplementation } from './types/tool';

export { createAuditMiddleware } from './middleware/AuditMiddleware';
export type { AuditEvent, AuditEventBase, AuditMiddlewareOptions, AuditState } from './middleware/AuditMiddleware';
export { createShortMemoryMiddleware } from './middleware/ShortMemoryMiddleware';
export type { ShortMemoryState } from './middleware/ShortMemoryMiddleware';

// Error types
export {
    DriverError,
    SchemaValidationError,
    WorkflowError,
    StreamError,
    ConfigurationError
} from './types/errors';

export type { ErrorType } from './types/errors';

// Configuration
export type { RunConfig } from './IrInterpreter';
