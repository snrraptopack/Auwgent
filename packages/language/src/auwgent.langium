grammar Auwgent


entry Model:
    (elements+=Element)*;

Element:
    agents+=Agent;

Agent:
    "agent" name=ID "{" 
        configs+=(
            InputConfig 
            | OutputConfig 
            | ToolConfig
            | AgentConfig
            | WorkFlowConfig
        )* 
    "}"
;

AgentConfig:
    defaultconfig=DefaultConfigModel (nondefaultConfig+=NonDefaultConfigModel*)?
;

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
    ("prompt" ":" prompt=STRING)?;

InputConfig:
    "input" "{" inProperties+=TypeConfigDeclaration* "}";
OutputConfig:
    "output" "{" outProperties+=(Output)* "}";

Output:
    td=TypeConfigDeclaration "@desc" description=STRING
;

ToolConfig:
    "tool" tool=ToolFunction
;


ToolFunction:
    name=ID "(" (params+=TypeConfigDeclaration ("," params+=TypeConfigDeclaration)*)? ")" 
    ":" returns=Types "{" "description" ":" desc=STRING "}";

Statement:
    VariableDeclartion | ReturnStatement | IfStatement | TypeDeclaration
;

VariableDeclartion:
    "let" name=ID "=" value=Expression
;

TypeDeclaration:
    "type" name=ID "{" types+=TypeConfigDeclaration* "}" ;

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

type Referenceable =  VariableDeclartion |TypeConfigDeclaration ;

Expression:
    {infer StringLiteral} value=STRING
    | {infer NumberLiteral} value=INT
    | {infer UnionLiteral} value=UnionType
    | {infer BooleanLiteral} value=("true"|"false")
    | ArrayLiteral
    | ObjectLiteral
    | {infer VariableRef} variable=[Referenceable]
    | FunctionCall
    | TemplateLiteral
;

FunctionCall:
    func=[ToolFunction:ID] "(" 
        (args+=Expression  ("," args+=Expression)*)?
    ")"
;



ArrayLiteral:
    '[' (elements+=Expression (',' elements+=Expression)*)? ']';

ObjectLiteral:
    '{' (properties+=PropertyValue (',' properties+=PropertyValue)*)? '}';


TemplateLiteral:
    "`" templates+=(TemplateString|TemplateExpr)*  "`"
    ;

TemplateString:
    value=ID;

TemplateExpr:
    "${" expr=Expression "}";

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

hidden terminal ML_COMMENT: /\/\*[\s\S]*?\*\//;
hidden terminal SL_COMMENT: /\/\/[^\n\r]*/;
