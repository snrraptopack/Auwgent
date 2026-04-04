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
    pass

MainToolsDict = Dict[str, Callable[..., Awaitable[Any]]]

# No custom intents defined
MainCustomIntents = None

class MainMainOutputResponseSchemaIntent(TypedDict):
    type: Literal["MainOutput"]
    response: MainOutput

MainResponseSchemaIntent = Union[MainMainOutputResponseSchemaIntent]

class MainResponseTextIntent(TypedDict):
    text: str

class MainErrorIntent(TypedDict):
    message: str
MainIntentValue = Union[
    MainResponseTextIntent,
    MainResponseSchemaIntent,
    MainErrorIntent,
]
MainIntentName = Literal["response_text", "response_schema", "error"]

class MainIntentControlSkip(TypedDict):
    skip: Literal[True]

class MainIntentControlOverride(TypedDict):
    result: Any

MainIntentControl = Union[MainIntentControlSkip, MainIntentControlOverride]
MainIntentHandlerReturn = Optional[Union[SessionState, MainIntentControl]]

MainIntentHandler = Callable[[MainIntentName, MainIntentValue, str], Awaitable[MainIntentHandlerReturn]]
MainPartialIntentHandler = Callable[[MainIntentName, PartialIntentValue, str], None]

class MainBaseIntentHandler:
    def response_text(self, intent: MainResponseTextIntent, agent_name: str) -> Union[MainIntentHandlerReturn, Awaitable[MainIntentHandlerReturn]]:
        pass
    def response_schema(self, intent: MainResponseSchemaIntent, agent_name: str) -> Union[MainIntentHandlerReturn, Awaitable[MainIntentHandlerReturn]]:
        pass
    def error(self, intent: MainErrorIntent, agent_name: str) -> Union[MainIntentHandlerReturn, Awaitable[MainIntentHandlerReturn]]:
        pass

class MainBasePartialIntentHandler:
    def response_text(self, intent: PartialTextIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def response_schema(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def error(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
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