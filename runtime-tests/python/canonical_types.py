# Auto-generated types for RuntimeTest
# Do not edit manually
import os
import json
from typing import TypedDict, Callable, Awaitable, Any, List, Dict, Union, Optional, Protocol, Literal, overload

# Required/NotRequired are 3.11+; fall back to typing_extensions for 3.9/3.10
try:
    from typing import Required, NotRequired
except ImportError:
    from typing_extensions import Required, NotRequired

try:
    from auwgent_sdk import TypedAuwgent, create_auwgent, Middleware, MiddlewareContext, SessionState, PartialIntentValue, PartialTextIntentValue, PartialStructuredIntentValue, AuwgentToolError, AuwgentTextPart, AuwgentImagePart, AuwgentFilePart, AuwgentAudioPart, AuwgentVideoPart
except ImportError:
    # For local testing if auwgent is not installed via pip
    import sys
    sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))
    from auwgent_sdk import TypedAuwgent, create_auwgent, Middleware, MiddlewareContext, SessionState, PartialIntentValue, PartialTextIntentValue, PartialStructuredIntentValue, AuwgentToolError, AuwgentTextPart, AuwgentImagePart, AuwgentFilePart, AuwgentAudioPart, AuwgentVideoPart

TextPart = AuwgentTextPart
ImagePart = AuwgentImagePart
FilePart = AuwgentFilePart
AudioPart = AuwgentAudioPart
VideoPart = AuwgentVideoPart
InputPart = Union[TextPart, ImagePart, FilePart, AudioPart, VideoPart]
Input = str

class PlannerOutput(TypedDict, total=False):
    steps: Required[List[str]]
    motivation: Required[str]

JokerOutput = None

class AuwgentOutput(TypedDict, total=False):
    pass

class AuwgentContext(TypedDict, total=False):
    user_name: Required[str]
    age: Required[float]
    id: Required[str]

class AuwgentTools(Protocol):
    # Return the current location for the active user
    async def get_location(self) -> str: ...

    # Return the user's score
    async def get_marks(self, id: str) -> str: ...

AuwgentToolsDict = Dict[str, Callable[..., Awaitable[Any]]]

class LoudIntent(TypedDict, total=False):
    actions: Required[str]
    reason: Required[str]

AuwgentCustomIntents = LoudIntent

class NoArgs(TypedDict, total=False):
    pass

GetLocationResult = str

class GetLocationToolCall(TypedDict):
    type: Literal["get_location"]

class GetLocationToolResult(TypedDict):
    name: Literal["get_location"]
    args: NoArgs
    result: GetLocationResult
    overridden: NotRequired[bool]

class GetLocationToolError(TypedDict):
    tool: Literal["get_location"]
    message: str

class GetLocationToolSkipped(TypedDict):
    type: Literal["get_location"]

class GetMarksToolArgs(TypedDict, total=False):
    id: Required[str]

GetMarksResult = str

class GetMarksToolCall(TypedDict):
    type: Literal["get_marks"]
    args: GetMarksToolArgs

class GetMarksToolResult(TypedDict):
    name: Literal["get_marks"]
    args: GetMarksToolArgs
    result: GetMarksResult
    overridden: NotRequired[bool]

class GetMarksToolError(TypedDict):
    tool: Literal["get_marks"]
    message: str

class GetMarksToolSkipped(TypedDict):
    type: Literal["get_marks"]
    args: GetMarksToolArgs

ToolCall = Union[GetLocationToolCall, GetMarksToolCall]
ToolResult = Union[GetLocationToolResult, GetMarksToolResult]
ToolError = Union[GetLocationToolError, GetMarksToolError]
ToolSkipped = Union[GetLocationToolSkipped, GetMarksToolSkipped]
ToolCalls = ToolCall
ToolResults = ToolResult
ToolErrors = ToolError
ToolSkippeds = ToolSkipped

class AuwgentOutputResponseSchema(TypedDict):
    type: Literal["AuwgentOutput"]
    response: AuwgentOutput

class PlannerOutputResponseSchema(TypedDict):
    type: Literal["PlannerOutput"]
    response: PlannerOutput

ResponseSchema = Union[AuwgentOutputResponseSchema, PlannerOutputResponseSchema]

class ResponseText(TypedDict):
    text: str

class ErrorIntent(TypedDict):
    message: str
class MarksAndLocationWorkflowArgs(TypedDict, total=False):
    user_id: Required[str]

MarksAndLocationWorkflowResultValue = str

class MarksAndLocationWorkflowCall(TypedDict):
    type: Literal["marks_and_location"]
    args: MarksAndLocationWorkflowArgs

class MarksAndLocationWorkflowResult(TypedDict):
    name: Literal["marks_and_location"]
    result: MarksAndLocationWorkflowResultValue

PlannerHelperArgs = str

class PlannerHelperResultValue(TypedDict, total=False):
    steps: Required[List[str]]
    motivation: Required[str]

class PlannerHelperCall(TypedDict):
    type: Literal["Planner"]
    args: PlannerHelperArgs

class PlannerHelperResult(TypedDict):
    name: Literal["Planner"]
    result: PlannerHelperResultValue

JokerHelperArgs = str

JokerHelperResultValue = TypedDict('_TextOutput', {"text": str}, total=False)

class JokerHelperCall(TypedDict):
    type: Literal["Joker"]
    args: JokerHelperArgs

class JokerHelperResult(TypedDict):
    name: Literal["Joker"]
    result: JokerHelperResultValue

AuwgentIntentValue = Union[
    ToolCall,
    ToolResult,
    ToolError,
    ToolSkipped,
    ResponseText,
    ResponseSchema,
    ErrorIntent,
    LoudIntent,
    MarksAndLocationWorkflowCall,
    MarksAndLocationWorkflowResult,
    PlannerHelperCall,
    PlannerHelperResult,
    JokerHelperCall,
    JokerHelperResult,
]
WorkflowCall = Union[MarksAndLocationWorkflowCall]
WorkflowResult = Union[MarksAndLocationWorkflowResult]
WorkflowCalls = WorkflowCall
WorkflowResults = WorkflowResult

HelperCall = Union[PlannerHelperCall, JokerHelperCall]
HelperResult = Union[PlannerHelperResult, JokerHelperResult]
HelperCalls = HelperCall
HelperResults = HelperResult

AuwgentIntentName = Literal["tool_call", "tool_result", "tool_error", "tool_skipped", "response_text", "response_schema", "error", "Loud", "workflow_call", "workflow_result", "helper_call", "helper_result"]

class AuwgentIntentControlSkip(TypedDict):
    skip: Literal[True]

class AuwgentIntentControlOverride(TypedDict):
    result: Any

AuwgentIntentControl = Union[AuwgentIntentControlSkip, AuwgentIntentControlOverride]
AuwgentIntentHandlerReturn = Optional[Union[SessionState, AuwgentIntentControl]]

AuwgentIntentHandler = Callable[[AuwgentIntentName, AuwgentIntentValue, str], Awaitable[AuwgentIntentHandlerReturn]]
# Partial intent payloads use top-level fields (for example: text/type/args/response).
AuwgentPartialResponseTextIntent = PartialTextIntentValue
AuwgentPartialResponseSchemaIntent = PartialStructuredIntentValue
AuwgentPartialErrorIntent = PartialStructuredIntentValue
AuwgentPartialToolCallIntent = PartialStructuredIntentValue
AuwgentPartialToolResultIntent = PartialStructuredIntentValue
AuwgentPartialToolErrorIntent = PartialStructuredIntentValue
AuwgentPartialToolSkippedIntent = PartialStructuredIntentValue
AuwgentPartialWorkflowCallIntent = PartialStructuredIntentValue
AuwgentPartialWorkflowResultIntent = PartialStructuredIntentValue
AuwgentPartialHelperCallIntent = PartialStructuredIntentValue
AuwgentPartialHelperResultIntent = PartialStructuredIntentValue
AuwgentPartialIntentHandler = Callable[[AuwgentIntentName, PartialIntentValue, str], None]

class AuwgentBaseIntentHandler:
    def tool_call(self, value: ToolCalls, agent_name: str) -> Union[AuwgentIntentHandlerReturn, Awaitable[AuwgentIntentHandlerReturn]]:
        pass
    def tool_result(self, value: ToolResults, agent_name: str) -> Union[AuwgentIntentHandlerReturn, Awaitable[AuwgentIntentHandlerReturn]]:
        pass
    def tool_error(self, value: ToolErrors, agent_name: str) -> Union[AuwgentIntentHandlerReturn, Awaitable[AuwgentIntentHandlerReturn]]:
        pass
    def tool_skipped(self, value: ToolSkippeds, agent_name: str) -> Union[AuwgentIntentHandlerReturn, Awaitable[AuwgentIntentHandlerReturn]]:
        pass
    def response_text(self, value: ResponseText, agent_name: str) -> Union[AuwgentIntentHandlerReturn, Awaitable[AuwgentIntentHandlerReturn]]:
        pass
    def response_schema(self, value: ResponseSchema, agent_name: str) -> Union[AuwgentIntentHandlerReturn, Awaitable[AuwgentIntentHandlerReturn]]:
        pass
    def error(self, value: ErrorIntent, agent_name: str) -> Union[AuwgentIntentHandlerReturn, Awaitable[AuwgentIntentHandlerReturn]]:
        pass
    def loud(self, value: LoudIntent, agent_name: str) -> Union[AuwgentIntentHandlerReturn, Awaitable[AuwgentIntentHandlerReturn]]:
        pass
    def workflow_call(self, value: WorkflowCalls, agent_name: str) -> Union[AuwgentIntentHandlerReturn, Awaitable[AuwgentIntentHandlerReturn]]:
        pass
    def workflow_result(self, value: WorkflowResults, agent_name: str) -> Union[AuwgentIntentHandlerReturn, Awaitable[AuwgentIntentHandlerReturn]]:
        pass
    def helper_call(self, value: HelperCalls, agent_name: str) -> Union[AuwgentIntentHandlerReturn, Awaitable[AuwgentIntentHandlerReturn]]:
        pass
    def helper_result(self, value: HelperResults, agent_name: str) -> Union[AuwgentIntentHandlerReturn, Awaitable[AuwgentIntentHandlerReturn]]:
        pass

class AuwgentBasePartialIntentHandler:
    def tool_call(self, value: AuwgentPartialToolCallIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def tool_result(self, value: AuwgentPartialToolResultIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def tool_error(self, value: AuwgentPartialToolErrorIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def tool_skipped(self, value: AuwgentPartialToolSkippedIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def response_text(self, value: AuwgentPartialResponseTextIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def response_schema(self, value: AuwgentPartialResponseSchemaIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def error(self, value: AuwgentPartialErrorIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def loud(self, value: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def workflow_call(self, value: AuwgentPartialWorkflowCallIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def workflow_result(self, value: AuwgentPartialWorkflowResultIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def helper_call(self, value: AuwgentPartialHelperCallIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def helper_result(self, value: AuwgentPartialHelperResultIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass

class AuwgentApiKeys(TypedDict, total=False):
    groqApiKey: str

class AuwgentAgent(TypedAuwgent[Any, AuwgentContext, AuwgentOutput, AuwgentTools]):
    def on_intent(self, handler: Union[AuwgentBaseIntentHandler, type[AuwgentBaseIntentHandler]]) -> None:
        return super().on_intent(handler)

    def on_intent_partial(self, handler: Union[AuwgentBasePartialIntentHandler, type[AuwgentBasePartialIntentHandler]]) -> None:
        return super().on_intent_partial(handler)

AuwgentMiddleware = Middleware

class AuwgentConfig(TypedDict, total=False):
    tools: NotRequired[Union['AuwgentTools', AuwgentToolsDict]]
    middleware: NotRequired[List[Union['AuwgentMiddleware', 'type[AuwgentMiddleware]']]]
    context: 'AuwgentContext'
    apiKeys: NotRequired['AuwgentApiKeys']

def createAuwgent(config: AuwgentConfig) -> 'AuwgentAgent':
    """Create a fully configured Auwgent agent from config."""
    ir_path = os.path.join(os.path.dirname(__file__), "canonical.agent.json")
    with open(ir_path, "r", encoding="utf-8") as f:
        ir_dict = json.load(f)
    return create_auwgent(ir_dict, config)

auwgent = createAuwgent