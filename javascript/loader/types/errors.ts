/**
 * Error types and classes for the Auwgent runtime
 */

/**
 * Classification of errors that can occur during agent execution
 */
export type ErrorType =
    | 'AUTH_ERROR'          // Invalid API key or authentication failure
    | 'RATE_LIMIT'          // 429 Too Many Requests
    | 'NETWORK_ERROR'       // Connection failed, timeout
    | 'INVALID_REQUEST'     // 400 Bad Request - malformed request
    | 'CONTENT_POLICY'      // Content filtered by provider
    | 'TOKEN_LIMIT'         // Context window exceeded
    | 'MODEL_NOT_FOUND'     // Invalid model name
    | 'UNKNOWN_ERROR';      // Unclassified error

/**
 * Base error class for driver-level errors
 */
export class DriverError extends Error {
    constructor(
        public readonly type: ErrorType,
        public readonly message: string,
        public readonly originalError: Error,
        public readonly retryable: boolean,
        public readonly statusCode?: number
    ) {
        super(message);
        this.name = 'DriverError';
        Object.setPrototypeOf(this, DriverError.prototype);
    }

    /**
     * Get a user-friendly error message
     */
    getUserMessage(): string {
        switch (this.type) {
            case 'AUTH_ERROR':
                return 'Authentication failed. Please check your API key.';
            case 'RATE_LIMIT':
                return 'Rate limit exceeded. Please try again later.';
            case 'NETWORK_ERROR':
                return 'Network error. Please check your connection and try again.';
            case 'INVALID_REQUEST':
                return 'Invalid request. Please check your input.';
            case 'CONTENT_POLICY':
                return 'Content was filtered by the provider\'s content policy.';
            case 'TOKEN_LIMIT':
                return 'Context window exceeded. Please reduce input size.';
            case 'MODEL_NOT_FOUND':
                return 'Model not found. Please check the model name.';
            default:
                return `An error occurred: ${this.message}`;
        }
    }
}

/**
 * Error thrown when model output doesn't match expected schema
 */
export class SchemaValidationError extends Error {
    constructor(
        public readonly output: unknown,
        public readonly expectedSchema: Record<string, unknown>,
        public readonly validationErrors: string[]
    ) {
        super('Model output does not match expected schema');
        this.name = 'SchemaValidationError';
        Object.setPrototypeOf(this, SchemaValidationError.prototype);
    }

    getUserMessage(): string {
        return `Schema validation failed:\n${this.validationErrors.join('\n')}`;
    }
}

/**
 * Error thrown when workflow execution fails
 */
export class WorkflowError extends Error {
    constructor(
        public readonly workflowName: string,
        public readonly stepName: string | undefined,
        public readonly originalError: Error
    ) {
        const stepInfo = stepName ? ` at step "${stepName}"` : '';
        super(`Workflow "${workflowName}" failed${stepInfo}: ${originalError.message}`);
        this.name = 'WorkflowError';
        Object.setPrototypeOf(this, WorkflowError.prototype);
    }

    getUserMessage(): string {
        const stepInfo = this.stepName ? ` (step: ${this.stepName})` : '';
        return `Workflow "${this.workflowName}" failed${stepInfo}: ${this.originalError.message}`;
    }
}

/**
 * Error thrown when streaming fails
 */
export class StreamError extends Error {
    constructor(
        public readonly phase: 'initialization' | 'streaming' | 'handler',
        public readonly originalError: Error
    ) {
        super(`Stream error during ${phase}: ${originalError.message}`);
        this.name = 'StreamError';
        Object.setPrototypeOf(this, StreamError.prototype);
    }

    getUserMessage(): string {
        return `Streaming failed during ${this.phase}: ${this.originalError.message}`;
    }
}

/**
 * Error thrown when agent configuration is invalid
 */
export class ConfigurationError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'ConfigurationError';
        Object.setPrototypeOf(this, ConfigurationError.prototype);
    }

    getUserMessage(): string {
        return this.message;
    }
}
