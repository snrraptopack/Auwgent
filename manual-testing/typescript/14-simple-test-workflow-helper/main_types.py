# Auto-generated types for Main
# Do not edit manually
import os
import json
from typing import TypedDict, Callable, Awaitable, Any, List, Dict, Union, Optional, Protocol, Literal, overload

# NotRequired is 3.11+; fall back to typing_extensions for 3.9/3.10
try:
    from typing import NotRequired
except ImportError:
    from typing_extensions import NotRequired

try:
    from auwgent_sdk import TypedAuwgent, create_auwgent, Middleware, MiddlewareContext, SessionState, PartialIntentValue, PartialTextIntentValue, PartialStructuredIntentValue, AuwgentToolError
except ImportError:
    # For local testing if auwgent is not installed via pip
    import sys
    sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))
    from auwgent_sdk import TypedAuwgent, create_auwgent, Middleware, MiddlewareContext, SessionState, PartialIntentValue, PartialTextIntentValue, PartialStructuredIntentValue, AuwgentToolError

class MainInput(TypedDict, total=False):
    pass

class MainOutput(TypedDict, total=False):
    pass

class MainContext(TypedDict, total=False):
    pass

class MainTools(Protocol):
    # 
    async def aa(self) -> float: ...

    # 
    async def bb(self) -> bool: ...

    # This tools is available to the helper only
    async def one(self) -> str: ...

MainToolsDict = Dict[str, Callable[..., Awaitable[Any]]]

# No custom intents defined
MainCustomIntents = None

class MainaaToolArgs(TypedDict, total=False):
    pass

MainaaToolResultValue = float

class MainaaToolCallIntent(TypedDict):
    type: Literal["aa"]
    args: MainaaToolArgs

class MainaaToolResultIntent(TypedDict):
    name: Literal["aa"]
    result: MainaaToolResultValue
    overridden: NotRequired[bool]

class MainaaToolErrorIntent(TypedDict):
    tool: Literal["aa"]
    message: str

class MainaaToolSkippedIntent(TypedDict):
    type: Literal["aa"]
    args: MainaaToolArgs

class MainbbToolArgs(TypedDict, total=False):
    pass

MainbbToolResultValue = bool

class MainbbToolCallIntent(TypedDict):
    type: Literal["bb"]
    args: MainbbToolArgs

class MainbbToolResultIntent(TypedDict):
    name: Literal["bb"]
    result: MainbbToolResultValue
    overridden: NotRequired[bool]

class MainbbToolErrorIntent(TypedDict):
    tool: Literal["bb"]
    message: str

class MainbbToolSkippedIntent(TypedDict):
    type: Literal["bb"]
    args: MainbbToolArgs

MainToolCallIntent = Union[MainaaToolCallIntent, MainbbToolCallIntent]
MainToolResultIntent = Union[MainaaToolResultIntent, MainbbToolResultIntent]
MainToolErrorIntent = Union[MainaaToolErrorIntent, MainbbToolErrorIntent]
MainToolSkippedIntent = Union[MainaaToolSkippedIntent, MainbbToolSkippedIntent]

class MainAOutputResponseSchemaIntent(TypedDict):
    type: Literal["AOutput"]
    response: AOutput

class MainBOutputResponseSchemaIntent(TypedDict):
    type: Literal["BOutput"]
    response: BOutput

class MainMainOutputResponseSchemaIntent(TypedDict):
    type: Literal["MainOutput"]
    response: MainOutput

MainResponseSchemaIntent = Union[MainAOutputResponseSchemaIntent, MainBOutputResponseSchemaIntent, MainMainOutputResponseSchemaIntent]

class MainResponseTextIntent(TypedDict):
    text: str

class MainErrorIntent(TypedDict):
    message: str
class MainanalyzeWorkflowArgs(TypedDict, total=False):
    pass

MainanalyzeWorkflowResultValue = str

class MainanalyzeWorkflowCall(TypedDict):
    type: Literal["analyze"]
    args: MainanalyzeWorkflowArgs

class MainanalyzeWorkflowResult(TypedDict):
    name: Literal["analyze"]
    result: MainanalyzeWorkflowResultValue

MainAHelperArgs = str

MainAHelperResultValue = Dict[str, Any]

class MainAHelperCall(TypedDict):
    type: Literal["A"]
    args: MainAHelperArgs

class MainAHelperResult(TypedDict):
    name: Literal["A"]
    result: MainAHelperResultValue

MainBHelperArgs = str

MainBHelperResultValue = TypedDict('_TextOutput', {"text": str}, total=False)

class MainBHelperCall(TypedDict):
    type: Literal["B"]
    args: MainBHelperArgs

class MainBHelperResult(TypedDict):
    name: Literal["B"]
    result: MainBHelperResultValue

MainIntentValue = Union[
    MainToolCallIntent,
    MainToolResultIntent,
    MainToolErrorIntent,
    MainToolSkippedIntent,
    MainResponseTextIntent,
    MainResponseSchemaIntent,
    MainErrorIntent,
    MainanalyzeWorkflowCall,
    MainanalyzeWorkflowResult,
    MainAHelperCall,
    MainAHelperResult,
    MainBHelperCall,
    MainBHelperResult,
]
MainWorkflowCallIntentValue = Union[MainanalyzeWorkflowCall]
MainWorkflowResultIntentValue = Union[MainanalyzeWorkflowResult]
MainHelperCallIntentValue = Union[MainAHelperCall, MainBHelperCall]
MainHelperResultIntentValue = Union[MainAHelperResult, MainBHelperResult]
MainIntentName = Literal["tool_call", "tool_result", "tool_error", "tool_skipped", "response_text", "response_schema", "error", "workflow_call", "workflow_result", "helper_call", "helper_result"]

class MainIntentControlSkip(TypedDict):
    skip: Literal[True]

class MainIntentControlOverride(TypedDict):
    result: Any

MainIntentControl = Union[MainIntentControlSkip, MainIntentControlOverride]
MainIntentHandlerReturn = Optional[Union[SessionState, MainIntentControl]]

MainIntentHandler = Callable[[MainIntentName, MainIntentValue, str], Awaitable[MainIntentHandlerReturn]]
MainPartialIntentHandler = Callable[[MainIntentName, PartialIntentValue, str], None]

class MainBaseIntentHandler:
    def tool_call(self, intent: MainToolCallIntent, agent_name: str) -> Union[MainIntentHandlerReturn, Awaitable[MainIntentHandlerReturn]]:
        pass
    def tool_result(self, intent: MainToolResultIntent, agent_name: str) -> Union[MainIntentHandlerReturn, Awaitable[MainIntentHandlerReturn]]:
        pass
    def tool_error(self, intent: MainToolErrorIntent, agent_name: str) -> Union[MainIntentHandlerReturn, Awaitable[MainIntentHandlerReturn]]:
        pass
    def tool_skipped(self, intent: MainToolSkippedIntent, agent_name: str) -> Union[MainIntentHandlerReturn, Awaitable[MainIntentHandlerReturn]]:
        pass
    def response_text(self, intent: MainResponseTextIntent, agent_name: str) -> Union[MainIntentHandlerReturn, Awaitable[MainIntentHandlerReturn]]:
        pass
    def response_schema(self, intent: MainResponseSchemaIntent, agent_name: str) -> Union[MainIntentHandlerReturn, Awaitable[MainIntentHandlerReturn]]:
        pass
    def error(self, intent: MainErrorIntent, agent_name: str) -> Union[MainIntentHandlerReturn, Awaitable[MainIntentHandlerReturn]]:
        pass
    def workflow_call(self, intent: Union[MainanalyzeWorkflowCall], agent_name: str) -> Union[MainIntentHandlerReturn, Awaitable[MainIntentHandlerReturn]]:
        pass
    def workflow_result(self, intent: Union[MainanalyzeWorkflowResult], agent_name: str) -> Union[MainIntentHandlerReturn, Awaitable[MainIntentHandlerReturn]]:
        pass
    def helper_call(self, intent: Union[MainAHelperCall, MainBHelperCall], agent_name: str) -> Union[MainIntentHandlerReturn, Awaitable[MainIntentHandlerReturn]]:
        pass
    def helper_result(self, intent: Union[MainAHelperResult, MainBHelperResult], agent_name: str) -> Union[MainIntentHandlerReturn, Awaitable[MainIntentHandlerReturn]]:
        pass

class MainBasePartialIntentHandler:
    def tool_call(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def tool_result(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def tool_error(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def tool_skipped(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def response_text(self, intent: PartialTextIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def response_schema(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def error(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def workflow_call(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def workflow_result(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def helper_call(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def helper_result(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass

class MainApiKeys(TypedDict, total=False):
    geminiApiKey: str

class MainAgent(TypedAuwgent[Any, MainContext, MainOutput, MainTools]):
    def on_intent(self, handler: Union[MainBaseIntentHandler, type[MainBaseIntentHandler]]) -> None:
        return super().on_intent(handler)

    def on_intent_partial(self, handler: Union[MainBasePartialIntentHandler, type[MainBasePartialIntentHandler]]) -> None:
        return super().on_intent_partial(handler)

MainMiddleware = Middleware

class MainConfig(TypedDict, total=False):
    tools: NotRequired[Union['MainTools', MainToolsDict]]
    middleware: NotRequired[List[Union['MainMiddleware', 'type[MainMiddleware]']]]
    apiKeys: NotRequired['MainApiKeys']

def createMain(config: MainConfig) -> 'MainAgent':
    """Create a fully configured Main agent from config."""
    ir_path = os.path.join(os.path.dirname(__file__), "main.agent.json")
    with open(ir_path, "r", encoding="utf-8") as f:
        ir_dict = json.load(f)
    return create_auwgent(ir_dict, config)

auwgent = createMain
AuwgentTools = MainTools
AuwgentConfig = MainConfig
AuwgentAgent = MainAgent
AuwgentMiddleware = MainMiddleware
AuwgentContext = MainContext
AuwgentIntentName = MainIntentName
AuwgentIntentValue = MainIntentValue
AuwgentIntentHandler = MainIntentHandler
AuwgentPartialIntentHandler = MainPartialIntentHandler
AuwgentBaseIntentHandler = MainBaseIntentHandler
AuwgentBasePartialIntentHandler = MainBasePartialIntentHandler