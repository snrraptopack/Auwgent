grammar Auwgent


entry Model:
    (imports+=FileImport)*
    (elements+=Element)*;

FileImport:
    'import' (NamedImports | WildcardImport) 'from' importPath=STRING;

NamedImports:
    '{' imports+=ImportSpecifier (',' imports+=ImportSpecifier)* '}';

ImportSpecifier:
    (kind=ImportKind)? imported=[Exportable:ID] ('as' alias=ID)?;

ImportKind returns string:
    'helper' | 'type' | 'prompt' | 'model';

WildcardImport:
    '*' 'as' namespace=ID;

Element:
    Agent | NamedPrompt | Helper | TypeDeclaration | ModelDefinition;

type Exportable = Helper | TypeDeclaration | NamedPrompt | ModelDefinition;
type Callable = ToolFunction | NamedPrompt;

Helper:
    exported?="export"? "helper" name=ID "{"
        "description" ":" desc=STRING
        configs+=AgentConfigurations* 
    "}"
;


Agent:
    "agent" name=ID "{" 
        configs+=(
           AgentConfigurations
        )* 
    "}"
;

AgentConfigurations:
    InputConfig 
    | OutputConfig 
    | ContextConfig
    | ToolConfig
    | ToolsConfig
    | AgentConfig
    | WorkFlowConfig
    | HelpersConfig
    | UseLifecycle
    | TestConfig
;

AgentConfig:
    defaultconfig=DefaultConfigModel (nondefaultConfig+=NonDefaultConfigModel*)?
;

HelpersConfig:
    "helpers" "{" helpers+=HelperRef* "}";

HelperRef:
    helper=[Helper]
    ("with" "tools" "{" (withAllTools?="all" | grantedTools+=[ToolFunction]* ) "}")?
    ("handoff" handoffUser?="user" (handoffThenContinue?="then" "continue")?)?;

TestConfig:
    "test" name=STRING "{"
        ("config" ":" configName=ID)?
        ("input" input=ObjectLiteral)?
        ("tools" "{" toolStubs+=ToolStub* "}")?
        ("expect" "{" expectations+=TestExpectation* "}")?
        ("model" "{" model=TestModel "}")?
    "}"
;

ToolStub:
    name=ID ":" ("returns" value=Expression | "error" error=STRING)
;

TestExpectation:
    "output" "." path+=ID ("." path+=ID)* ":" value=Expression
    | "tool_error" ":" error=STRING
    | "prompt_contains" ":" contains=STRING
;

TestModel:
    ("toolCalls" "{" toolCalls+=TestToolCall* "}")?
    ("finalText" ":" finalText=STRING)?
;

TestToolCall:
    name=ID "(" (args+=PropertyValue ("," args+=PropertyValue)*)? ")"
;

HelperCall:
    "hlp" "." helper=[Helper:ID] "(" (args+=Expression ("," args+=Expression)*)? ")";

WorkFlowConfig:
    "workflow" name=ID 
        "(" (params+=TypeConfigDeclaration ("," params+=TypeConfigDeclaration)*)? ")" 
        ":" return=Types "{"
        "description" ":" desc=STRING
        (workflowToolConfigs+=ToolConfig | workflowToolsConfigs+=ToolsConfig)*
        (body+=Statement)*
    "}"
;

DefaultConfigModel:
    "default" "config" "{" ModelConfig "}"
;
NonDefaultConfigModel:
    "config" name=ID  "{" nonConf=ModelConfig "}"
;

ModelConfig:
    "model" ":" model=ModelProviderRef
    ("prompt" "{" (parts+=PromptStatement)* "}")?
    ("prompt" ":" promptExpr=Expression)?
;

ModelProviderRef:
    ModelProvider | ModelRef
;

ModelProvider:
    GeminiProvider | OpenAIProvider | CustomProvider
;

ModelRef:
    ref=[ModelDefinition:ID]
;

GeminiProvider:
    "gemini" "(" modelName=STRING ("," config=ObjectLiteral)? ")"
;

OpenAIProvider:
    "openai" "(" modelName=STRING ("," config=ObjectLiteral)? ")"
;

CustomProvider:
    "custom" "(" url=STRING "," modelName=STRING ("," config=ObjectLiteral)? ")"
;

ModelDefinition:
    exported?="export"? "model" name=ID "{"
        "provider" ":" provider=ModelProvider
    "}"
;

NamedPrompt:
    exported?="export"? "prompt" name=ID 
        ("(" (params+=TypeConfigDeclaration ("," params+=TypeConfigDeclaration)*)? ")")?
        "{" (parts+=PromptStatement)* "}";

PromptStatement:
    ExampleBlock | Expression | IfStatement;

ExampleBlock:
    "example" "{"
        (messages+=ExampleMessage)*
    "}";

ExampleMessage:
    role=("user"|"assistant") ":" text=STRING;

InputConfig:
    "input" "{" (inProperties+=TypeConfigDeclaration (","? inProperties+=TypeConfigDeclaration)*)? "}";

OutputConfig:
    "output" ("{" (outProperties+=Output (","? outProperties+=Output)*)? "}" | ":" directType=Types ("@desc" directTypeDesc=STRING)?);

ContextConfig:
    "context" "{" (contextProperties+=TypeConfigDeclaration (","? contextProperties+=TypeConfigDeclaration)*)? "}"
;
UseLifecycle:
    "use" "lifecycle" ("{"
        ("maxTokens" ":" maxTokens=INT)?
        ("maxMessages" ":" maxMessages=INT)?
    "}")?
;

ContextReference:
    'ctx' '.' property=[TypeConfigDeclaration:ID];

Output:
    td=TypeConfigDeclaration ("@desc" description=STRING)?
;

ToolConfig:
    "tool" tool=ToolFunction
;

ToolsConfig:
    "tools" "{" tools+=ToolFunction* "}";

ToolFunction:
    name=ID 
    "(" (params+=TypeConfigDeclaration ("," params+=TypeConfigDeclaration)*)? ")" 
    ":" returns=Types 
    ("{" "description" ":" desc+=STRING "}")?
    ("@desc" desc+=STRING)?
;

Statement:
    VariableDeclartion | ReturnStatement | IfStatement | TransferStatement | ParallelStatement
;

VariableDeclartion:
    "let" name=ID "=" value=Expression
;

TypeDeclaration:
    exported?="export"? (isOutput?="output")? "type" name=ID "{" (types+=TypeConfigDeclaration (","? types+=TypeConfigDeclaration)*)? "}";

TypeConfigDeclaration:
    name=ID (isOptional?='?')? ":" t=Types ("@desc" description=STRING)?;

IfStatement:
    'if' '(' condition=Condition ')' '{' 
        (thenBlock+=Statement)* 
    '}' 
    ('else' '{' 
        (elseBlock+=Statement)* 
    '}')?
;

Condition:
    left=Expression op=Operators right=Expression;

Operators returns string:
    "=="| ">" | "<" | "!="
;

ReturnStatement:
    "return" value=Expression
;

ParallelStatement:
    "parallel" "{"
        (body+=Statement)*
    "}"
;

// NamedPrompt is included so prompt names can be referenced directly (e.g., myPrompt + "text")
type Referenceable = VariableDeclartion | TypeConfigDeclaration | NamedPrompt;

// Binary expression with left recursion for + operator
Expression:
    AdditionExpression;

AdditionExpression infers Expression:
    PrimaryExpression ({infer BinaryExpression.left=current} op='+' right=PrimaryExpression)*;

PrimaryExpression infers Expression:
    {infer StringLiteral} value=STRING
    | {infer MultilineStringLiteral} value=MULTILINE_STRING
    | {infer NumberLiteral} value=INT
    | {infer BooleanLiteral} value=("true"|"false")
    | ArrayLiteral
    | ObjectLiteral
    | MemberAccess
    | {infer VariableRef} variable=[Referenceable]
    | PromptCall
    | FunctionCall
    | TemplateLiteral
    | ContextReference
    | HelperCall
    | InlinePromptBlock
    | '(' Expression ')'
;

// Inline prompt parts: { "text" if(...) "more" }
InlinePromptBlock:
    '{' (parts+=PromptStatement)* '}';

MemberAccess:
    object=[Referenceable] '.' property=ID ('.' chain+=ID)*
;

FunctionCall:
    func=[Callable:ID] "(" 
        (args+=Expression  ("," args+=Expression)*)?
    ")"
;

PromptCall:
    "prompt" prompt=[NamedPrompt:ID] "("
        (args+=Expression ("," args+=Expression)*)?
    ")"
;

TransferStatement:
    "transfer" "to" call=HelperCall (thenContinue?="then" "continue")?;

ArrayLiteral:
    '[' (elements+=Expression (',' elements+=Expression)*)? ']';

ObjectLiteral:
    '{' (properties+=PropertyValue (',' properties+=PropertyValue)*)? '}';


TemplateLiteral:
    "`"  templates+=(TemplateString|TemplateExpr)*  "`"
;

TemplateString:
    value=(ID|STRING);

TemplateExpr:
    "{" expr=Expression "}";

PropertyValue:
    name=ID (':' value=Expression)?;


Types:
    BaseType ('[' ']' {infer ArrayType.elementType=current})*;

BaseType:
    type=(StringType | NumberType | BooleanType | ObjectType | UnionType)
    | typeRef=[TypeDeclaration:ID];

BooleanType:
    type="boolean";
StringType:
    type="string";
NumberType:
    type="number";
UnionType:
    options+=STRING ('|' options+=STRING)*;

ObjectType:
    '{' (properties+=PropertyType (','? properties+=PropertyType)*)? '}';

PropertyType:
    name=ID (isOptional?='?')? ":" type=Types ("@desc" description=STRING)?;


hidden terminal WS: /\s+/;
terminal ID: /[_a-zA-Z][\w_]*/;


terminal INT returns number:/[0-9]+(\.[0-9]+)?/;
terminal MULTILINE_STRING: /"""[\s\S]*?"""/;
terminal STRING: /"(\\.|[^"\\])*"|'(\\.|[^'\\])*'/;
terminal TEMPLATE: /`[^`]*`/;

hidden terminal ML_COMMENT: /\/\*[\s\S]*?\*\//;
hidden terminal SL_COMMENT: /\/\/[^\n\r]*/;
