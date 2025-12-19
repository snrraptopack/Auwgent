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

TypeConfigDeclaration:
    name=ID ":" t=Types;


Types:
    types=(UnionType|BooleanType|StringType|NumberType|INT|ArrayType);

Statement:
    VariableDeclartion | ReturnStatement
;

VariableDeclartion:
    "let" name=ID "=" value=Expression
;

ReturnStatement:
    "return" value=Expression
;

Expression:
    {infer StringLiteral} value=STRING
    | {infer NumberLiteral} value=INT
    | {infer UnionLiteral} value=UnionType
    | ArrayLiteral
    | VariableRef
    | FunctionCall
;

FunctionCall:
    func=[ToolFunction:ID] "(" 
        (args+=Expression  ("," args+=Expression)*)?
    ")"
;

VariableRef:
    variable=[VariableDeclartion:ID]
;

ArrayLiteral:
    '[' (elements+=Expression (',' elements+=Expression)*)? ']';

ArrayType:
    type=(StringType | NumberType | BooleanType) '[' ']';
BooleanType:
    type="boolean";
StringType:
    type="string";
NumberType:
    type="number";
UnionType:
    options+=STRING ('|' options+=STRING)*;


hidden terminal WS: /\s+/;
terminal ID: /[_a-zA-Z][\w_]*/;
terminal INT returns number: /[0-9]+/;
terminal STRING: /"(\\.|[^"\\])*"|'(\\.|[^'\\])*'/;
hidden terminal ML_COMMENT: /\/\*[\s\S]*?\*\//;
hidden terminal SL_COMMENT: /\/\/[^\n\r]*/;
