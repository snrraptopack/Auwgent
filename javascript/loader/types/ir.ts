export interface AgentIR {
    name: string;
    modelConfig: ModelConfig[];
    input: Record<string, string>;
    output: Record<string, string | { type: string; description: string, optional: boolean }>;
    context: Record<string, string>
    tools: Tool[];
    workflows: Workflow[];
    helpers: HelperIR[];
    helperToolGrants?: Record<string, string[] | "all">;  // helperName -> tool names or "all"
}

export interface HelperIR {
    name: string;
    description: string;
    modelConfig: ModelConfig[];
    input: Record<string, string>;
    output: Record<string, string | { type: string; description: string, optional: boolean }>;
    context: Record<string, string>;
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
    | TransferStatement
    | Expression;

export type Expression =
    | Literal
    | UnionLiteral
    | VarRef
    | FunctionCall
    | HelperCall
    | ObjectLiteral
    | ArrayLiteral
    | TemplateLiteral
    | ContextReference
    | MemberAccess

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

export interface HelperCall {
    type: "helperCall";
    value: string; // Helper name
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
    value: TemplatePart[];
}

export type TemplatePart =
    | { type: "literal"; value: string }
    | { type: "expression"; value: Expression };

export interface ContextReference {
    type: "contextRef",
    property: string
}

export interface MemberAccess {
    type: "memberAccess";
    object: VarRef;
    properties: string[];  // Chain of property names, e.g., ["propose"] or ["data", "result"]
}

export interface TransferStatement {
    type: "transfer";
    target: HelperCall;
    mode: "direct" | "thenContinue";
}

export type ModelConfig = {
    defaultConfig: Config,
    namedConfig: Config[]
}

export type Config = {
    modelName: string,
    prompt: PromptConfig | null
}

export type PromptConfig =
    | { type: "simple", value: string }
    | { type: "ref", name?: string, value: Expression[] }
    | { type: "parts", value: Expression[] }