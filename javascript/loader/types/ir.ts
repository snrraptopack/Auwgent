export interface AgentIR {
    name: string;
    modelConfig: ModelConfig[];
    input: Record<string, TypeInfo>;
    output: Record<string, TypeInfo>;
    context: Record<string, TypeInfo>;
    tools: Tool[];
    workflows: Workflow[];
    helpers: HelperIR[];
    helperToolGrants?: Record<string, string[] | "all">;
    helperHandoff?: Record<string, "user" | "thenContinue">;
    types?: Record<string, TypeDefinition>;
    tests?: AgentTest[];
}

export interface HelperIR {
    name: string;
    description: string;
    modelConfig: ModelConfig[];
    input: Record<string, TypeInfo>;
    output: Record<string, TypeInfo>;
    context: Record<string, TypeInfo>;
    tools: Tool[];
    workflows: Workflow[];
}

export interface Tool {
    name: string;
    description: string;
    params: Record<string, TypeInfo>;
    returns: IRType;
}

export interface Workflow {
    flowName: string;
    flowParams: Record<string, TypeInfo>;
    returns: IRType;
    body: Statement[];
    description: string;
    tools?: Tool[];
}

export interface AgentTest {
    name: string;
    configName?: string;
    input?: Expression;
    toolStubs?: TestToolStub[];
    expectations?: TestExpectation[];
    model?: TestModel;
}

export interface TestToolStub {
    name: string;
    returns?: Expression;
    error?: string;
}

export type TestExpectation =
    | { type: "output"; path: string[]; value: Expression }
    | { type: "tool_error"; error: string }
    | { type: "prompt_contains"; contains: string };

export interface TestModel {
    toolCalls?: TestToolCall[];
    finalText?: string;
}

export interface TestToolCall {
    name: string;
    args?: Expression;
}

export type Statement =
    | VariableDeclaration
    | AssignmentStatement
    | ReturnStatement
    | IfStatement
    | TransferStatement
    | ParallelStatement
    | Expression;

export type Expression =
    | Literal
    | UnionLiteral
    | VarRef
    | FunctionCall
    | WorkflowCall
    | HelperCall
    | PromptRef
    | ObjectLiteral
    | ArrayLiteral
    | ContextReference
    | MemberAccess
    | IndexAccess
    | ConcatExpression
    | BinaryOpExpression
    | TemplateExpression
    | ExpressionWrapper;

export interface VariableDeclaration {
    type: "variableDeclaration";
    name: string;
    value: Expression;
}

export interface AssignmentStatement {
    type: "assignment";
    name: string;
    value: Expression;
}

export interface ReturnStatement {
    type: "return";
    value: Expression;
}

// Condition types for if statements
export type Condition = ComparisonCondition | LogicalCondition | BooleanCondition;

export interface ComparisonCondition {
    type: "comparison";
    left: Expression;
    operator: "==" | "!=" | ">" | "<" | ">=" | "<=";
    right: Expression;
}

export interface LogicalCondition {
    type: "logical";
    operator: "&&" | "||";
    left: Condition;
    right: Condition;
}

export interface BooleanCondition {
    type: "boolean";
    value: Expression;
}

export interface IfStatement {
    type: "if";
    condition: Condition;
    then: Statement[];
    else: Statement[];
}

export interface ParallelStatement {
    type: "parallel";
    body: Statement[];
}

export interface IndexAccess {
    type: "indexAccess";
    object: string;
    index: Expression;
    property?: string;
    chain?: string[];
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

export interface WorkflowCall {
    type: "workflowCall";
    value: string; // Workflow name
    args: Expression[];
}

export interface HelperCall {
    type: "helperCall";
    value: string; // Helper name
    args: Expression[];
}

export interface PromptRef {
    type: "promptRef";
    name?: string;
    params?: string[];
    args?: Expression[];
    value: Expression[];
}

export interface ObjectLiteral {
    type: "object";
    value: Record<string, Expression>;  // Properties map to expressions
}

export interface ArrayLiteral {
    type: "array";
    value: Expression[];  // Array of expressions
}

export interface TemplateExpression {
    type: "template";
    value: TemplatePart[];
}

export type TemplatePart = Literal | ExpressionWrapper;

export interface ExpressionWrapper {
    type: "expression";
    value: Expression;
}

export interface ContextReference {
    type: "contextRef",
    property: string
}

export interface MemberAccess {
    type: "memberAccess";
    object: VarRef;
    properties: string[];  // Chain of property names, e.g., ["propose"] or ["data", "result"]
}

export interface ConcatExpression {
    type: "concat";
    left: Expression;
    right: Expression;
}

export interface BinaryOpExpression {
    type: "binaryOp";
    op: "+" | "-" | "*" | "/";
    left: Expression;
    right: Expression;
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

export type ModelProvider =
    | { type: "gemini"; modelName: string; config?: Expression }
    | { type: "openai"; modelName: string; config?: Expression }
    | { type: "custom"; url: string; modelName: string; config?: Expression };

export type Config = {
    configName?: string;
    model: ModelProvider;
    prompt: PromptConfig | null;
}

export type PromptConfig =
    | { type: "simple", value: string }
    | { type: "ref", name?: string, value: Expression[] }
    | { type: "parts", value: Expression[] }

// Enhanced Type System Types

/**
 * Represents type information with optional flag
 */
export interface TypeInfo {
    type: IRType;
    optional: boolean;
    description?: string;
}

/**
 * IR Type representation - can be a primitive string or a complex type object
 */
export type IRType =
    | string                    // Primitive types: "string", "number", "boolean"
    | ArrayTypeIR               // Array types
    | TypeRefIR                 // Type references
    | UnionTypeIR               // Union types
    | ObjectTypeIR;             // Inline object types

/**
 * Array type: { type: "array", items: IRType }
 */
export interface ArrayTypeIR {
    type: "array";
    items: IRType;
}

/**
 * Type reference: { type: "typeRef", name: string }
 */
export interface TypeRefIR {
    type: "typeRef";
    name: string;
}

/**
 * Union type: { type: "union", options: string[] }
 */
export interface UnionTypeIR {
    type: "union";
    options: string[];
}

/**
 * Inline object type: { type: "object", properties: Record<string, IRType> }
 */
export interface ObjectTypeIR {
    type: "object";
    properties: Record<string, IRType>;
}

/**
 * Type definition in the types section
 */
export interface TypeDefinition {
    isOutput: boolean;
    properties: Record<string, PropertyInfo>;
}

/**
 * Property information within a type definition
 */
export interface PropertyInfo {
    type: IRType;
    optional: boolean;
    description?: string;
}
