# Auto-generated from auwgent-ir-schema — do not edit manually.
# Source of truth: auwgent-compiler/crates/auwgent-ir-schema/src/lib.rs
from __future__ import annotations
from typing import Any, Dict, List, Literal, Optional, Union
try:
    from typing import TypedDict, NotRequired
except ImportError:
    from typing_extensions import TypedDict, NotRequired

class ModelConfigBlockIR(TypedDict, total=False):
    defaultConfig: NotRequired[Optional[ModelConfigIR]]
    namedConfig: NotRequired[Optional[List[Any]]]

class ModelConfigIR(TypedDict, total=False):
    model: ModelProviderIR
    embedding: NotRequired[Optional[ModelProviderIR]]
    prompt: Any

class ModelProviderIR(TypedDict, total=False):
    pass

class NamedModelConfigIR(TypedDict, total=False):
    configName: str
    model: ModelProviderIR
    embedding: NotRequired[Optional[ModelProviderIR]]
    prompt: Any

class ToolIR(TypedDict, total=False):
    name: str
    description: NotRequired[Optional[str]]
    params: Any
    returns: Any
    examples: NotRequired[List[Any]]

class WorkflowIR(TypedDict, total=False):
    flowName: str
    flowParams: NotRequired[Any]
    returns: Any
    description: NotRequired[Optional[str]]
    body: NotRequired[List[Any]]
    tools: NotRequired[List[Any]]
    examples: NotRequired[List[Any]]

class ExpressionIR(TypedDict, total=False):
    pass

class ExamplePairIR(TypedDict, total=False):
    user: str
    assistant: str

class HelperIR(TypedDict, total=False):
    name: str
    description: NotRequired[Optional[str]]
    modelConfig: NotRequired[List[Any]]
    input: NotRequired[Any]
    output: NotRequired[Any]
    context: NotRequired[Any]
    tools: NotRequired[List[Any]]
    workflows: NotRequired[List[Any]]
    customIntents: NotRequired[Optional[List[Any]]]
    examples: NotRequired[List[Any]]

class CustomIntentIR(TypedDict, total=False):
    name: str
    description: NotRequired[Optional[str]]
    fields: Any
    examples: NotRequired[List[Any]]

class TypeDeclIR(TypedDict, total=False):
    isOutput: bool
    properties: Dict[str, Any]

class TypePropertyIR(TypedDict, total=False):
    type: Any
    optional: bool
    description: NotRequired[Optional[str]]

class AgentIR(TypedDict, total=False):
    name: str
    modelConfig: NotRequired[List[Any]]
    input: NotRequired[Any]
    output: NotRequired[Any]
    context: NotRequired[Any]
    tools: NotRequired[List[Any]]
    workflows: NotRequired[List[Any]]
    helpers: NotRequired[List[Any]]
    types: NotRequired[Optional[Dict[str, Any]]]
    helperToolGrants: NotRequired[Optional[Dict[str, Any]]]
    helperHandoff: NotRequired[Optional[Dict[str, Any]]]
    tests: NotRequired[List[Any]]
    lifecycle: NotRequired[Any]
    customIntents: NotRequired[Optional[List[Any]]]