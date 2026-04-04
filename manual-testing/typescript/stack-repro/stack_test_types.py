# Auto-generated types for Router
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

class RouterInput(TypedDict, total=False):
    pass

class StoryTellerOutput(TypedDict, total=False):
    pass

class AnalyzerOutput(TypedDict, total=False):
    type: Dict[str, Any]

class RouterBaseOutput(TypedDict, total=False):
    pass

RouterOutput = Union[RouterBaseOutput, StoryTellerOutput, AnalyzerOutput]

class RouterContext(TypedDict, total=False):
    user_name: str

class RouterTools(Protocol):
    pass

RouterToolsDict = Dict[str, Callable[..., Awaitable[Any]]]

RouterCustomIntents = Union[
    TypedDict('_thoughtCustomIntent', {"name": Literal["thought"], "value": {"explain": str}}, total=False),
    TypedDict('_questionsCustomIntent', {"name": Literal["questions"], "value": {"questions": str}}, total=False),
]

class RouterAnalyzerOutputResponseSchemaIntent(TypedDict):
    type: Literal["AnalyzerOutput"]
    response: AnalyzerOutput

class RouterRouterOutputResponseSchemaIntent(TypedDict):
    type: Literal["RouterOutput"]
    response: RouterOutput

class RouterStoryTellerOutputResponseSchemaIntent(TypedDict):
    type: Literal["StoryTellerOutput"]
    response: StoryTellerOutput

RouterResponseSchemaIntent = Union[RouterAnalyzerOutputResponseSchemaIntent, RouterRouterOutputResponseSchemaIntent, RouterStoryTellerOutputResponseSchemaIntent]

class RouterResponseTextIntent(TypedDict):
    text: str

class RouterErrorIntent(TypedDict):
    message: str
class RouterthoughtCustomIntent(TypedDict):
    name: Literal["thought"]
    value: {"explain": str}

class RouterquestionsCustomIntent(TypedDict):
    name: Literal["questions"]
    value: {"questions": str}

RouterStoryTellerHelperArgs = Dict[str, Any]

RouterStoryTellerHelperResultValue = TypedDict('_TextOutput', {"text": str}, total=False)

class RouterStoryTellerHelperCall(TypedDict):
    type: Literal["StoryTeller"]
    args: RouterStoryTellerHelperArgs

class RouterStoryTellerHelperResult(TypedDict):
    name: Literal["StoryTeller"]
    result: RouterStoryTellerHelperResultValue

class RouterAnalyzerHelperArgs(TypedDict, total=False):
    text: str

RouterAnalyzerHelperResultValue = Dict[str, Any]

class RouterAnalyzerHelperCall(TypedDict):
    type: Literal["Analyzer"]
    args: RouterAnalyzerHelperArgs

class RouterAnalyzerHelperResult(TypedDict):
    name: Literal["Analyzer"]
    result: RouterAnalyzerHelperResultValue

RouterIntentValue = Union[
    RouterResponseTextIntent,
    RouterResponseSchemaIntent,
    RouterErrorIntent,
    RouterthoughtCustomIntent,
    RouterquestionsCustomIntent,
    RouterStoryTellerHelperCall,
    RouterStoryTellerHelperResult,
    RouterAnalyzerHelperCall,
    RouterAnalyzerHelperResult,
]
RouterHelperCallIntentValue = Union[RouterStoryTellerHelperCall, RouterAnalyzerHelperCall]
RouterHelperResultIntentValue = Union[RouterStoryTellerHelperResult, RouterAnalyzerHelperResult]
RouterIntentName = Literal["response_text", "response_schema", "error", "thought", "questions", "helper_call", "helper_result"]

class RouterIntentControlSkip(TypedDict):
    skip: Literal[True]

class RouterIntentControlOverride(TypedDict):
    result: Any

RouterIntentControl = Union[RouterIntentControlSkip, RouterIntentControlOverride]
RouterIntentHandlerReturn = Optional[Union[SessionState, RouterIntentControl]]

RouterIntentHandler = Callable[[RouterIntentName, RouterIntentValue, str], Awaitable[RouterIntentHandlerReturn]]
RouterPartialIntentHandler = Callable[[RouterIntentName, PartialIntentValue, str], None]

class RouterBaseIntentHandler:
    def response_text(self, intent: RouterResponseTextIntent, agent_name: str) -> Union[RouterIntentHandlerReturn, Awaitable[RouterIntentHandlerReturn]]:
        pass
    def response_schema(self, intent: RouterResponseSchemaIntent, agent_name: str) -> Union[RouterIntentHandlerReturn, Awaitable[RouterIntentHandlerReturn]]:
        pass
    def error(self, intent: RouterErrorIntent, agent_name: str) -> Union[RouterIntentHandlerReturn, Awaitable[RouterIntentHandlerReturn]]:
        pass
    def thought(self, intent: RouterthoughtCustomIntent, agent_name: str) -> Union[RouterIntentHandlerReturn, Awaitable[RouterIntentHandlerReturn]]:
        pass
    def questions(self, intent: RouterquestionsCustomIntent, agent_name: str) -> Union[RouterIntentHandlerReturn, Awaitable[RouterIntentHandlerReturn]]:
        pass
    def helper_call(self, intent: Union[RouterStoryTellerHelperCall, RouterAnalyzerHelperCall], agent_name: str) -> Union[RouterIntentHandlerReturn, Awaitable[RouterIntentHandlerReturn]]:
        pass
    def helper_result(self, intent: Union[RouterStoryTellerHelperResult, RouterAnalyzerHelperResult], agent_name: str) -> Union[RouterIntentHandlerReturn, Awaitable[RouterIntentHandlerReturn]]:
        pass

class RouterBasePartialIntentHandler:
    def response_text(self, intent: PartialTextIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def response_schema(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def error(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def thought(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def questions(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def helper_call(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def helper_result(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass

class RouterApiKeys(TypedDict, total=False):
    geminiApiKey: str
    my_groq_apiApiKey: str  # API key for custom provider 'my-groq-api'

class RouterAgent(TypedAuwgent[Any, RouterContext, RouterOutput, RouterTools]):
    def on_intent(self, handler: Union[RouterBaseIntentHandler, type[RouterBaseIntentHandler]]) -> None:
        return super().on_intent(handler)

    def on_intent_partial(self, handler: Union[RouterBasePartialIntentHandler, type[RouterBasePartialIntentHandler]]) -> None:
        return super().on_intent_partial(handler)

RouterMiddleware = Middleware

class RouterConfig(TypedDict, total=False):
    middleware: NotRequired[List[Union['RouterMiddleware', 'type[RouterMiddleware]']]]
    context: NotRequired['RouterContext']
    apiKeys: NotRequired['RouterApiKeys']

def createRouter(config: RouterConfig) -> 'RouterAgent':
    """Create a fully configured Router agent from config."""
    ir_path = os.path.join(os.path.dirname(__file__), "stack_test.agent.json")
    with open(ir_path, "r", encoding="utf-8") as f:
        ir_dict = json.load(f)
    return create_auwgent(ir_dict, config)

auwgent = createRouter
AuwgentTools = RouterTools
AuwgentConfig = RouterConfig
AuwgentAgent = RouterAgent
AuwgentMiddleware = RouterMiddleware
AuwgentContext = RouterContext
AuwgentIntentName = RouterIntentName
AuwgentIntentValue = RouterIntentValue
AuwgentIntentHandler = RouterIntentHandler
AuwgentPartialIntentHandler = RouterPartialIntentHandler
AuwgentBaseIntentHandler = RouterBaseIntentHandler
AuwgentBasePartialIntentHandler = RouterBasePartialIntentHandler