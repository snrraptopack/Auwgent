export interface AgentIR {
    name: string;
    modelConfig: ModelConfig[];
    input: Record<string, string>;
    output: Record<string, string | { type: string; description: string, optional: boolean }>;
    tools: Tool[];
    workflows: Workflow[];
}

export interface Tool {
    name: string;
    description: string;
    params: Record<string, string>;
    returns: string;
}

export interface Workflow {
    flowName: string;
    flowParams: Record<string, string>;
    returns: string;
    body: Statement[];
    description: string;
}

export type Statement =
    | VariableDeclaration
    | ReturnStatement
    | IfStatement
    | Expression;

export type Expression =
    | Literal
    | UnionLiteral
    | VarRef
    | FunctionCall
    | ObjectLiteral
    | ArrayLiteral
    | TemplateLiteral

export interface VariableDeclaration {
    type: "variableDeclaration";
    name: string;
    value: Expression;
}

export interface ReturnStatement {
    type: "return";
    value: Expression;
}

export interface IfStatement {
    type: "if";
    condition: {
        left: Expression;
        operator: string;
        right: Expression;
    };
    then: Statement[];
    else: Statement[];
}

export interface Literal {
    type: "literal";
    value: string | number | boolean;
}

export interface UnionLiteral {
    type: "union";
    value: string[];
}

export interface VarRef {
    type: "varRef";
    value: string; // The variable name
}

export interface FunctionCall {
    type: "functionCall";
    value: string; // Function name
    args: Expression[];
}

export interface ObjectLiteral {
    type: "object";
    value: Record<string, Expression>;  // Properties map to expressions
}

export interface ArrayLiteral {
    type: "array";
    value: Expression[];  // Array of expressions
}

export interface TemplateLiteral {
    type: "template";
    parts: TemplatePart[];
}

export type TemplatePart = 
    | { type: "literal"; value: string }
    | { type: "expression"; value: Expression };

export type ModelConfig = {
    defaultConfig: Config,
    namedConfig: Config[]
}

export type Config = {
    modelName: string,
    prompt: string | null
}