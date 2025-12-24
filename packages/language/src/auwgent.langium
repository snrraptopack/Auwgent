grammar Auwgent


entry Model:
    (elements+=Element)*;

Element:
    Agent | NamedPrompt | Helper;

Helper:
    "helper" name=ID "{"
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
;

AgentConfig:
    defaultconfig=DefaultConfigModel (nondefaultConfig+=NonDefaultConfigModel*)?
;

HelpersConfig:
    "helpers" "{" helpers+=HelperRef* "}";

HelperRef:
    helper=[Helper] ("with" "tools" "{" (withAllTools?="all" | grantedTools+=[ToolFunction]* ) "}")?;

HelperCall:
    "hlp" "." helper=[Helper:ID] "(" (args+=Expression ("," args+=Expression)*)? ")";

WorkFlowConfig:
    "workflow" name=ID 
        "(" (params+=TypeConfigDeclaration ("," params+=TypeConfigDeclaration)*)? ")" 
        ":" return=Types "{"
        "description" ":" desc=STRING
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
    "model" ":" ModelName=STRING
    ("prompt" "{" (parts+=PromptStatement)* "}")?
    ("prompt" ":" refPrompt=[NamedPrompt]?  simplePrompt=STRING?)?
;

NamedPrompt:
    "prompt" name=ID  "{" (parts+=PromptStatement)* "}";

PromptStatement:
    Expression|IfStatement;

InputConfig:
    "input" "{" inProperties+=TypeConfigDeclaration* "}";
OutputConfig:
    "output" "{" outProperties+=(Output)* "}";
ContextConfig:
    "context" "{" contextProperties+=TypeConfigDeclaration* "}"
;

ContextReference:
    'ctx' '.' property=[TypeConfigDeclaration:ID];

Output:
    td=TypeConfigDeclaration "@desc" description=STRING
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
    VariableDeclartion | ReturnStatement | IfStatement | TypeDeclaration | TransferStatement
;

VariableDeclartion:
    "let" name=ID "=" value=Expression
;

TypeDeclaration:
    "type" name=ID "{" types+=TypeConfigDeclaration* "}";

TypeConfigDeclaration:
    name=ID (isOptional?='?')? ":" t=Types;

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

type Referenceable = VariableDeclartion |TypeConfigDeclaration;

Expression:
    {infer StringLiteral} value=STRING
    | {infer NumberLiteral} value=INT
    | {infer BooleanLiteral} value=("true"|"false")
    | ArrayLiteral
    | ObjectLiteral
    | MemberAccess
    | {infer VariableRef} variable=[Referenceable]
    | FunctionCall
    | TemplateLiteral
    | ContextReference
    | HelperCall
;

MemberAccess:
    object=[Referenceable] '.' property=ID ('.' chain+=ID)*
;

FunctionCall:
    func=[ToolFunction:ID] "(" 
        (args+=Expression  ("," args+=Expression)*)?
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

ArrayType:
    type=(StringType | NumberType | BooleanType) '[' ']';

Types:
    types=(ArrayType|ObjectType|UnionType|BooleanType|StringType|NumberType|INT);

BooleanType:
    type="boolean";
StringType:
    type="string";
NumberType:
    type="number";
UnionType:
    options+=STRING ('|' options+=STRING)*;

ObjectType:
    '{' (properties+=PropertyType (',' properties+=PropertyType)*)? '}';

PropertyType:
    name=ID (isOptional?='?')? ":" type=Types;


hidden terminal WS: /\s+/;
terminal ID: /[_a-zA-Z][\w_]*/;


terminal INT returns number:/[0-9]+(\.[0-9]+)?/;
terminal STRING: /"(\\.|[^"\\])*"|'(\\.|[^'\\])*'/;
terminal TEMPLATE: /`[^`]*`/;

hidden terminal ML_COMMENT: /\/\*[\s\S]*?\*\//;
hidden terminal SL_COMMENT: /\/\/[^\n\r]*/;
