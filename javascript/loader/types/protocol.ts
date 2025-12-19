/**
 * specific implementation of JSON Schema for our runtime.
 * This is the "Assembly Language" for structured outputs.
 */
export interface JsonSchema {
    type?: string;
    description?: string;
    properties?: Record<string, JsonSchema>;
    required?: string[];
    items?: JsonSchema;
    enum?: string[];
    anyOf?: JsonSchema[];
    // We can add more JSON schema fields as needed
}


export interface SyntheticToolDef {
    name: string;
    description: string;
    parameters: JsonSchema;
}

/**
 * A normalized message format.
 * Drivers map this to their specific SDK message types.
 */
export interface SyntheticMessage {
    role: 'system' | 'user' | 'assistant';
    content: string;
}

/**
 * The Normalized LLM Interaction Object (NLIO).
 * This contains EVERYTHING a driver needs to execute a request.
 */
export interface SyntheticRequest {
    /** The strict conversation history */
    messages: SyntheticMessage[];

    /**
     * The schema for the expected response.
     * If present, the driver MUST enforce this structure.
     */
    responseSchema?: JsonSchema;


    //The tools available to the model
    tools?: SyntheticToolDef[];


    /** Model configuration hints */
    config: {
        model?: string;
        temperature?: number;
    };
}

export interface DriverResult {
    text?: string;
    toolParams?: {
        name: string;
        args: any;
    };
}

/**
 * The interface every provider driver must implement.
 */
export interface AgentDriver {
    name: string;
    /**
     * Execute the synthetic request and return the raw text result.
     * The driver is responsible for unwrapping the provider's specific response object.
     */
    execute(request: SyntheticRequest): Promise<DriverResult>;
}