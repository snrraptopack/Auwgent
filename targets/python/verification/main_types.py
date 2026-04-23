# Auto-generated types for Hello
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

class Person(TypedDict, total=False):
    age: float
    name: str
class HelloInput(TypedDict, total=False):
    pass

class HelloOutput(TypedDict, total=False):
    type: Dict[str, Any]

class HelloContext(TypedDict, total=False):
    pass

class HelloTools(Protocol):
    # use this to get the user's name and age
    async def get_user_name_age(self) -> "Person": ...

    # use this to ge location
    async def get_location(self) -> str: ...

HelloToolsDict = Dict[str, Callable[..., Awaitable[Any]]]

# No custom intents defined
HelloCustomIntents = None

class Helloget_user_name_ageToolArgs(TypedDict, total=False):
    pass

Helloget_user_name_ageToolResultValue = "Person"

class Helloget_user_name_ageToolCallIntent(TypedDict):
    type: Literal["get_user_name_age"]
    args: Helloget_user_name_ageToolArgs

class Helloget_user_name_ageToolResultIntent(TypedDict):
    name: Literal["get_user_name_age"]
    result: Helloget_user_name_ageToolResultValue
    overridden: NotRequired[bool]

class Helloget_user_name_ageToolErrorIntent(TypedDict):
    tool: Literal["get_user_name_age"]
    message: str

class Helloget_user_name_ageToolSkippedIntent(TypedDict):
    type: Literal["get_user_name_age"]
    args: Helloget_user_name_ageToolArgs

class Helloget_locationToolArgs(TypedDict, total=False):
    pass

Helloget_locationToolResultValue = str

class Helloget_locationToolCallIntent(TypedDict):
    type: Literal["get_location"]
    args: Helloget_locationToolArgs

class Helloget_locationToolResultIntent(TypedDict):
    name: Literal["get_location"]
    result: Helloget_locationToolResultValue
    overridden: NotRequired[bool]

class Helloget_locationToolErrorIntent(TypedDict):
    tool: Literal["get_location"]
    message: str

class Helloget_locationToolSkippedIntent(TypedDict):
    type: Literal["get_location"]
    args: Helloget_locationToolArgs

HelloToolCallIntent = Union[Helloget_user_name_ageToolCallIntent, Helloget_locationToolCallIntent]
HelloToolResultIntent = Union[Helloget_user_name_ageToolResultIntent, Helloget_locationToolResultIntent]
HelloToolErrorIntent = Union[Helloget_user_name_ageToolErrorIntent, Helloget_locationToolErrorIntent]
HelloToolSkippedIntent = Union[Helloget_user_name_ageToolSkippedIntent, Helloget_locationToolSkippedIntent]

class HelloHelloOutputResponseSchemaIntent(TypedDict):
    type: Literal["HelloOutput"]
    response: HelloOutput

class HelloPersonResponseSchemaIntent(TypedDict):
    type: Literal["Person"]
    response: Person

HelloResponseSchemaIntent = Union[HelloHelloOutputResponseSchemaIntent, HelloPersonResponseSchemaIntent]

class HelloResponseTextIntent(TypedDict):
    text: str

class HelloErrorIntent(TypedDict):
    message: str
HelloIntentValue = Union[
    HelloToolCallIntent,
    HelloToolResultIntent,
    HelloToolErrorIntent,
    HelloToolSkippedIntent,
    HelloResponseTextIntent,
    HelloResponseSchemaIntent,
    HelloErrorIntent,
]
HelloIntentName = Literal["tool_call", "tool_result", "tool_error", "tool_skipped", "response_text", "response_schema", "error"]

class HelloIntentControlSkip(TypedDict):
    skip: Literal[True]

class HelloIntentControlOverride(TypedDict):
    result: Any

HelloIntentControl = Union[HelloIntentControlSkip, HelloIntentControlOverride]
HelloIntentHandlerReturn = Optional[Union[SessionState, HelloIntentControl]]

HelloIntentHandler = Callable[[HelloIntentName, HelloIntentValue, str], Awaitable[HelloIntentHandlerReturn]]
# Partial intent payloads use top-level fields (for example: text/type/args/response).
HelloPartialResponseTextIntent = PartialTextIntentValue
HelloPartialResponseSchemaIntent = PartialStructuredIntentValue
HelloPartialErrorIntent = PartialStructuredIntentValue
HelloPartialToolCallIntent = PartialStructuredIntentValue
HelloPartialToolResultIntent = PartialStructuredIntentValue
HelloPartialToolErrorIntent = PartialStructuredIntentValue
HelloPartialToolSkippedIntent = PartialStructuredIntentValue
HelloPartialIntentHandler = Callable[[HelloIntentName, PartialIntentValue, str], None]

class HelloBaseIntentHandler:
    def tool_call(self, intent: HelloToolCallIntent, agent_name: str) -> Union[HelloIntentHandlerReturn, Awaitable[HelloIntentHandlerReturn]]:
        pass
    def tool_result(self, intent: HelloToolResultIntent, agent_name: str) -> Union[HelloIntentHandlerReturn, Awaitable[HelloIntentHandlerReturn]]:
        pass
    def tool_error(self, intent: HelloToolErrorIntent, agent_name: str) -> Union[HelloIntentHandlerReturn, Awaitable[HelloIntentHandlerReturn]]:
        pass
    def tool_skipped(self, intent: HelloToolSkippedIntent, agent_name: str) -> Union[HelloIntentHandlerReturn, Awaitable[HelloIntentHandlerReturn]]:
        pass
    def response_text(self, intent: HelloResponseTextIntent, agent_name: str) -> Union[HelloIntentHandlerReturn, Awaitable[HelloIntentHandlerReturn]]:
        pass
    def response_schema(self, intent: HelloResponseSchemaIntent, agent_name: str) -> Union[HelloIntentHandlerReturn, Awaitable[HelloIntentHandlerReturn]]:
        pass
    def error(self, intent: HelloErrorIntent, agent_name: str) -> Union[HelloIntentHandlerReturn, Awaitable[HelloIntentHandlerReturn]]:
        pass

class HelloBasePartialIntentHandler:
    def tool_call(self, intent: HelloPartialToolCallIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def tool_result(self, intent: HelloPartialToolResultIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def tool_error(self, intent: HelloPartialToolErrorIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def tool_skipped(self, intent: HelloPartialToolSkippedIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def response_text(self, intent: HelloPartialResponseTextIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def response_schema(self, intent: HelloPartialResponseSchemaIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def error(self, intent: HelloPartialErrorIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass

class HelloApiKeys(TypedDict, total=False):
    groqApiKey: str

class HelloAgent(TypedAuwgent[Any, HelloContext, HelloOutput, HelloTools]):
    def on_intent(self, handler: Union[HelloBaseIntentHandler, type[HelloBaseIntentHandler]]) -> None:
        return super().on_intent(handler)

    def on_intent_partial(self, handler: Union[HelloBasePartialIntentHandler, type[HelloBasePartialIntentHandler]]) -> None:
        return super().on_intent_partial(handler)

HelloMiddleware = Middleware

class HelloConfig(TypedDict, total=False):
    tools: NotRequired[Union['HelloTools', HelloToolsDict]]
    middleware: NotRequired[List[Union['HelloMiddleware', 'type[HelloMiddleware]']]]
    apiKeys: NotRequired['HelloApiKeys']

def createHello(config: HelloConfig) -> 'HelloAgent':
    """Create a fully configured Hello agent from config."""
    ir_path = os.path.join(os.path.dirname(__file__), "main.agent.json")
    with open(ir_path, "r", encoding="utf-8") as f:
        ir_dict = json.load(f)
    return create_auwgent(ir_dict, config)

auwgent = createHello
AuwgentTools = HelloTools
AuwgentConfig = HelloConfig
AuwgentAgent = HelloAgent
AuwgentMiddleware = HelloMiddleware
AuwgentContext = HelloContext
AuwgentIntentName = HelloIntentName
AuwgentIntentValue = HelloIntentValue
AuwgentIntentHandler = HelloIntentHandler
AuwgentPartialIntentHandler = HelloPartialIntentHandler
AuwgentBaseIntentHandler = HelloBaseIntentHandler
AuwgentBasePartialIntentHandler = HelloBasePartialIntentHandler