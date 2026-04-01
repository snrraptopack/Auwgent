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
    from auwgent_sdk import TypedAuwgent, create_auwgent, Middleware, MiddlewareContext, SessionState, AuwgentToolError
except ImportError:
    # For local testing if auwgent is not installed via pip
    import sys
    sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))
    from auwgent_sdk import TypedAuwgent, create_auwgent, Middleware, MiddlewareContext, SessionState, AuwgentToolError

class Order(TypedDict, total=False):
    product_id: str
    quantity: float
    total: float
    status: str
    user_id: str
    id: str

class QueryResult(TypedDict, total=False):
    message: str
    success: bool
    data: str

class AnalysisReport(TypedDict, total=False):
    insights: List[str]
    revenue: float
    total_users: float
    total_orders: float
    total_products: float

class Product(TypedDict, total=False):
    id: str
    name: str
    price: float
    stock: float

class User(TypedDict, total=False):
    email: str
    name: str
    created_at: str
    id: str
class MainInput(TypedDict, total=False):
    pass

class DataAnalyzerOutput(TypedDict, total=False):
    total_users: float
    total_products: float
    total_orders: float
    revenue: float
    insights: List[str]

class ReportGeneratorOutput(TypedDict, total=False):
    type: Dict[str, Any]

class MainBaseOutput(TypedDict, total=False):
    success: bool
    data: str
    message: str

MainOutput = Union[MainBaseOutput, DataAnalyzerOutput, ReportGeneratorOutput]

class MainContext(TypedDict, total=False):
    is_vip: bool
    user_id: str
    session_id: str

class MainTools(Protocol):
    # Query users from in-memory DB. Filter can be 'all', 'id:<id>', 'email:<email>'
    async def db_query_users(self, *, filter: str) -> List["User"]: ...

    # Query products from in-memory DB. Filter can be 'all', 'id:<id>', 'name:<name>'
    async def db_query_products(self, *, filter: str) -> List["Product"]: ...

    # Query orders from in-memory DB. Filter can be 'all', 'user_id:<id>', 'status:<status>'
    async def db_query_orders(self, *, filter: str) -> List["Order"]: ...

    # Create a new user in the database
    async def db_create_user(self, *, name: str, email: str) -> "User": ...

    # Create a new product in the database
    async def db_create_product(self, *, name: str, price: float, stock: float) -> "Product": ...

    # Create a new order in the database
    async def db_create_order(self, *, user_id: str, product_id: str, quantity: float) -> "Order": ...

    # Parse orders JSON and sum totals
    async def sum_order_totals(self, *, orders_json: str) -> float: ...

    # Check if enough stock available
    async def validate_stock(self, *, product_id: str, quantity: float) -> bool: ...

    # Parse comma-separated values
    async def parse_csv(self, *, csv_string: str) -> str: ...

    # Analyze a specific user's purchasing behavior
    async def analyze_user_behavior(self, *, user_id: str) -> str: ...

    # Find products with low stock levels
    async def detect_low_stock(self) -> str: ...

    # Calculate average from comma-separated numbers
    async def calculate_average(self, *, numbers: str) -> float: ...

    # Identify outliers in dataset
    async def find_outliers(self, *, data: str) -> str: ...

    # Format data as a text table
    async def format_table(self, *, data: str) -> str: ...

    # Generate a textual description of a chart
    async def generate_chart_description(self, *, data: str, chart_type: str) -> str: ...

    # Group orders by status and count
    async def aggregate_by_status(self, *, orders: str) -> str: ...

    # Calculate key sales metrics
    async def calculate_metrics(self, *, orders: str) -> str: ...

MainToolsDict = Dict[str, Callable[..., Awaitable[Any]]]

MainCustomIntents = TypedDict('_SpeakLoudCustomIntent', {"name": Literal["SpeakLoud"], "value": {"explain": str}}, total=False)

class Maindb_query_usersToolArgs(TypedDict, total=False):
    filter: str

Maindb_query_usersToolResultValue = List["User"]

class Maindb_query_usersToolCallIntent(TypedDict):
    type: Literal["db_query_users"]
    args: Maindb_query_usersToolArgs

class Maindb_query_usersToolResultIntent(TypedDict):
    name: Literal["db_query_users"]
    result: Maindb_query_usersToolResultValue
    overridden: NotRequired[bool]

class Maindb_query_usersToolErrorIntent(TypedDict):
    tool: Literal["db_query_users"]
    message: str

class Maindb_query_usersToolSkippedIntent(TypedDict):
    type: Literal["db_query_users"]
    args: Maindb_query_usersToolArgs

class Maindb_query_productsToolArgs(TypedDict, total=False):
    filter: str

Maindb_query_productsToolResultValue = List["Product"]

class Maindb_query_productsToolCallIntent(TypedDict):
    type: Literal["db_query_products"]
    args: Maindb_query_productsToolArgs

class Maindb_query_productsToolResultIntent(TypedDict):
    name: Literal["db_query_products"]
    result: Maindb_query_productsToolResultValue
    overridden: NotRequired[bool]

class Maindb_query_productsToolErrorIntent(TypedDict):
    tool: Literal["db_query_products"]
    message: str

class Maindb_query_productsToolSkippedIntent(TypedDict):
    type: Literal["db_query_products"]
    args: Maindb_query_productsToolArgs

class Maindb_query_ordersToolArgs(TypedDict, total=False):
    filter: str

Maindb_query_ordersToolResultValue = List["Order"]

class Maindb_query_ordersToolCallIntent(TypedDict):
    type: Literal["db_query_orders"]
    args: Maindb_query_ordersToolArgs

class Maindb_query_ordersToolResultIntent(TypedDict):
    name: Literal["db_query_orders"]
    result: Maindb_query_ordersToolResultValue
    overridden: NotRequired[bool]

class Maindb_query_ordersToolErrorIntent(TypedDict):
    tool: Literal["db_query_orders"]
    message: str

class Maindb_query_ordersToolSkippedIntent(TypedDict):
    type: Literal["db_query_orders"]
    args: Maindb_query_ordersToolArgs

class Maindb_create_userToolArgs(TypedDict, total=False):
    name: str
    email: str

Maindb_create_userToolResultValue = "User"

class Maindb_create_userToolCallIntent(TypedDict):
    type: Literal["db_create_user"]
    args: Maindb_create_userToolArgs

class Maindb_create_userToolResultIntent(TypedDict):
    name: Literal["db_create_user"]
    result: Maindb_create_userToolResultValue
    overridden: NotRequired[bool]

class Maindb_create_userToolErrorIntent(TypedDict):
    tool: Literal["db_create_user"]
    message: str

class Maindb_create_userToolSkippedIntent(TypedDict):
    type: Literal["db_create_user"]
    args: Maindb_create_userToolArgs

class Maindb_create_productToolArgs(TypedDict, total=False):
    name: str
    price: float
    stock: float

Maindb_create_productToolResultValue = "Product"

class Maindb_create_productToolCallIntent(TypedDict):
    type: Literal["db_create_product"]
    args: Maindb_create_productToolArgs

class Maindb_create_productToolResultIntent(TypedDict):
    name: Literal["db_create_product"]
    result: Maindb_create_productToolResultValue
    overridden: NotRequired[bool]

class Maindb_create_productToolErrorIntent(TypedDict):
    tool: Literal["db_create_product"]
    message: str

class Maindb_create_productToolSkippedIntent(TypedDict):
    type: Literal["db_create_product"]
    args: Maindb_create_productToolArgs

class Maindb_create_orderToolArgs(TypedDict, total=False):
    user_id: str
    product_id: str
    quantity: float

Maindb_create_orderToolResultValue = "Order"

class Maindb_create_orderToolCallIntent(TypedDict):
    type: Literal["db_create_order"]
    args: Maindb_create_orderToolArgs

class Maindb_create_orderToolResultIntent(TypedDict):
    name: Literal["db_create_order"]
    result: Maindb_create_orderToolResultValue
    overridden: NotRequired[bool]

class Maindb_create_orderToolErrorIntent(TypedDict):
    tool: Literal["db_create_order"]
    message: str

class Maindb_create_orderToolSkippedIntent(TypedDict):
    type: Literal["db_create_order"]
    args: Maindb_create_orderToolArgs

MainToolCallIntent = Union[Maindb_query_usersToolCallIntent, Maindb_query_productsToolCallIntent, Maindb_query_ordersToolCallIntent, Maindb_create_userToolCallIntent, Maindb_create_productToolCallIntent, Maindb_create_orderToolCallIntent]
MainToolResultIntent = Union[Maindb_query_usersToolResultIntent, Maindb_query_productsToolResultIntent, Maindb_query_ordersToolResultIntent, Maindb_create_userToolResultIntent, Maindb_create_productToolResultIntent, Maindb_create_orderToolResultIntent]
MainToolErrorIntent = Union[Maindb_query_usersToolErrorIntent, Maindb_query_productsToolErrorIntent, Maindb_query_ordersToolErrorIntent, Maindb_create_userToolErrorIntent, Maindb_create_productToolErrorIntent, Maindb_create_orderToolErrorIntent]
MainToolSkippedIntent = Union[Maindb_query_usersToolSkippedIntent, Maindb_query_productsToolSkippedIntent, Maindb_query_ordersToolSkippedIntent, Maindb_create_userToolSkippedIntent, Maindb_create_productToolSkippedIntent, Maindb_create_orderToolSkippedIntent]

class MainAnalysisReportResponseSchemaIntent(TypedDict):
    type: Literal["AnalysisReport"]
    response: AnalysisReport

class MainDataAnalyzerOutputResponseSchemaIntent(TypedDict):
    type: Literal["DataAnalyzerOutput"]
    response: DataAnalyzerOutput

class MainMainOutputResponseSchemaIntent(TypedDict):
    type: Literal["MainOutput"]
    response: MainOutput

class MainOrderResponseSchemaIntent(TypedDict):
    type: Literal["Order"]
    response: Order

class MainProductResponseSchemaIntent(TypedDict):
    type: Literal["Product"]
    response: Product

class MainQueryResultResponseSchemaIntent(TypedDict):
    type: Literal["QueryResult"]
    response: QueryResult

class MainReportGeneratorOutputResponseSchemaIntent(TypedDict):
    type: Literal["ReportGeneratorOutput"]
    response: ReportGeneratorOutput

class MainUserResponseSchemaIntent(TypedDict):
    type: Literal["User"]
    response: User

MainResponseSchemaIntent = Union[MainAnalysisReportResponseSchemaIntent, MainDataAnalyzerOutputResponseSchemaIntent, MainMainOutputResponseSchemaIntent, MainOrderResponseSchemaIntent, MainProductResponseSchemaIntent, MainQueryResultResponseSchemaIntent, MainReportGeneratorOutputResponseSchemaIntent, MainUserResponseSchemaIntent]

class MainResponseTextIntent(TypedDict):
    text: str

class MainErrorIntent(TypedDict):
    message: str
class MainSpeakLoudCustomIntent(TypedDict):
    name: Literal["SpeakLoud"]
    value: {"explain": str}

class Mainget_user_ordersWorkflowArgs(TypedDict, total=False):
    target_user_id: str

Mainget_user_ordersWorkflowResultValue = str

class Mainget_user_ordersWorkflowCall(TypedDict):
    type: Literal["get_user_orders"]
    args: Mainget_user_ordersWorkflowArgs

class Mainget_user_ordersWorkflowResult(TypedDict):
    name: Literal["get_user_orders"]
    result: Mainget_user_ordersWorkflowResultValue

class Maincalculate_revenueWorkflowArgs(TypedDict, total=False):
    pass

Maincalculate_revenueWorkflowResultValue = str

class Maincalculate_revenueWorkflowCall(TypedDict):
    type: Literal["calculate_revenue"]
    args: Maincalculate_revenueWorkflowArgs

class Maincalculate_revenueWorkflowResult(TypedDict):
    name: Literal["calculate_revenue"]
    result: Maincalculate_revenueWorkflowResultValue

class Mainprocess_bulk_orderWorkflowArgs(TypedDict, total=False):
    target_user_id: str
    product_ids: str
    quantities: str

Mainprocess_bulk_orderWorkflowResultValue = str

class Mainprocess_bulk_orderWorkflowCall(TypedDict):
    type: Literal["process_bulk_order"]
    args: Mainprocess_bulk_orderWorkflowArgs

class Mainprocess_bulk_orderWorkflowResult(TypedDict):
    name: Literal["process_bulk_order"]
    result: Mainprocess_bulk_orderWorkflowResultValue

MainDataAnalyzerHelperArgs = Dict[str, Any]

class MainDataAnalyzerHelperResultValue(TypedDict, total=False):
    total_users: float
    total_products: float
    total_orders: float
    revenue: float
    insights: List[str]

class MainDataAnalyzerHelperCall(TypedDict):
    type: Literal["DataAnalyzer"]
    args: MainDataAnalyzerHelperArgs

class MainDataAnalyzerHelperResult(TypedDict):
    name: Literal["DataAnalyzer"]
    result: MainDataAnalyzerHelperResultValue

MainReportGeneratorHelperArgs = Dict[str, Any]

MainReportGeneratorHelperResultValue = Dict[str, Any]

class MainReportGeneratorHelperCall(TypedDict):
    type: Literal["ReportGenerator"]
    args: MainReportGeneratorHelperArgs

class MainReportGeneratorHelperResult(TypedDict):
    name: Literal["ReportGenerator"]
    result: MainReportGeneratorHelperResultValue

MainIntentValue = Union[
    MainToolCallIntent,
    MainToolResultIntent,
    MainToolErrorIntent,
    MainToolSkippedIntent,
    MainResponseTextIntent,
    MainResponseSchemaIntent,
    MainErrorIntent,
    MainSpeakLoudCustomIntent,
    Mainget_user_ordersWorkflowCall,
    Mainget_user_ordersWorkflowResult,
    Maincalculate_revenueWorkflowCall,
    Maincalculate_revenueWorkflowResult,
    Mainprocess_bulk_orderWorkflowCall,
    Mainprocess_bulk_orderWorkflowResult,
    MainDataAnalyzerHelperCall,
    MainDataAnalyzerHelperResult,
    MainReportGeneratorHelperCall,
    MainReportGeneratorHelperResult,
]
MainWorkflowCallIntentValue = Union[Mainget_user_ordersWorkflowCall, Maincalculate_revenueWorkflowCall, Mainprocess_bulk_orderWorkflowCall]
MainWorkflowResultIntentValue = Union[Mainget_user_ordersWorkflowResult, Maincalculate_revenueWorkflowResult, Mainprocess_bulk_orderWorkflowResult]
MainHelperCallIntentValue = Union[MainDataAnalyzerHelperCall, MainReportGeneratorHelperCall]
MainHelperResultIntentValue = Union[MainDataAnalyzerHelperResult, MainReportGeneratorHelperResult]
MainIntentName = Literal["tool_call", "tool_result", "tool_error", "tool_skipped", "response_text", "response_schema", "error", "SpeakLoud", "workflow_call", "workflow_result", "helper_call", "helper_result"]

MainIntentHandler = Callable[[MainIntentName, MainIntentValue, str], Awaitable[Optional[SessionState]]]
MainPartialIntentHandler = Callable[[MainIntentName, MainIntentValue, str], None]

class MainBaseIntentHandler:
    def tool_call(self, intent: MainToolCallIntent, agent_name: str) -> Union[Optional[SessionState], Awaitable[Optional[SessionState]]]:
        pass
    def tool_result(self, intent: MainToolResultIntent, agent_name: str) -> Union[Optional[SessionState], Awaitable[Optional[SessionState]]]:
        pass
    def tool_error(self, intent: MainToolErrorIntent, agent_name: str) -> Union[Optional[SessionState], Awaitable[Optional[SessionState]]]:
        pass
    def tool_skipped(self, intent: MainToolSkippedIntent, agent_name: str) -> Union[Optional[SessionState], Awaitable[Optional[SessionState]]]:
        pass
    def response_text(self, intent: MainResponseTextIntent, agent_name: str) -> Union[Optional[SessionState], Awaitable[Optional[SessionState]]]:
        pass
    def response_schema(self, intent: MainResponseSchemaIntent, agent_name: str) -> Union[Optional[SessionState], Awaitable[Optional[SessionState]]]:
        pass
    def error(self, intent: MainErrorIntent, agent_name: str) -> Union[Optional[SessionState], Awaitable[Optional[SessionState]]]:
        pass
    def speakloud(self, intent: MainSpeakLoudCustomIntent, agent_name: str) -> Union[Optional[SessionState], Awaitable[Optional[SessionState]]]:
        pass
    def workflow_call(self, intent: Union[Mainget_user_ordersWorkflowCall, Maincalculate_revenueWorkflowCall, Mainprocess_bulk_orderWorkflowCall], agent_name: str) -> Union[Optional[SessionState], Awaitable[Optional[SessionState]]]:
        pass
    def workflow_result(self, intent: Union[Mainget_user_ordersWorkflowResult, Maincalculate_revenueWorkflowResult, Mainprocess_bulk_orderWorkflowResult], agent_name: str) -> Union[Optional[SessionState], Awaitable[Optional[SessionState]]]:
        pass
    def helper_call(self, intent: Union[MainDataAnalyzerHelperCall, MainReportGeneratorHelperCall], agent_name: str) -> Union[Optional[SessionState], Awaitable[Optional[SessionState]]]:
        pass
    def helper_result(self, intent: Union[MainDataAnalyzerHelperResult, MainReportGeneratorHelperResult], agent_name: str) -> Union[Optional[SessionState], Awaitable[Optional[SessionState]]]:
        pass

class MainBasePartialIntentHandler:
    def tool_call(self, intent: MainToolCallIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def tool_result(self, intent: MainToolResultIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def tool_error(self, intent: MainToolErrorIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def tool_skipped(self, intent: MainToolSkippedIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def response_text(self, intent: MainResponseTextIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def response_schema(self, intent: MainResponseSchemaIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def error(self, intent: MainErrorIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def speakloud(self, intent: MainSpeakLoudCustomIntent, agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def workflow_call(self, intent: Union[Mainget_user_ordersWorkflowCall, Maincalculate_revenueWorkflowCall, Mainprocess_bulk_orderWorkflowCall], agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def workflow_result(self, intent: Union[Mainget_user_ordersWorkflowResult, Maincalculate_revenueWorkflowResult, Mainprocess_bulk_orderWorkflowResult], agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def helper_call(self, intent: Union[MainDataAnalyzerHelperCall, MainReportGeneratorHelperCall], agent_name: str) -> Union[None, Awaitable[None]]:
        pass
    def helper_result(self, intent: Union[MainDataAnalyzerHelperResult, MainReportGeneratorHelperResult], agent_name: str) -> Union[None, Awaitable[None]]:
        pass

class MainApiKeys(TypedDict, total=False):
    my_groq_apiApiKey: str  # API key for custom provider 'my-groq-api'

class MainAgent(TypedAuwgent[Any, MainContext, MainOutput, MainTools]):
    def on_intent(self, handler: MainBaseIntentHandler) -> None:
        return super().on_intent(handler)

    def on_intent_partial(self, handler: MainBasePartialIntentHandler) -> None:
        return super().on_intent_partial(handler)

MainMiddleware = Middleware

class MainConfig(TypedDict, total=False):
    tools: NotRequired[Union['MainTools', MainToolsDict]]
    middleware: NotRequired[List[Union['MainMiddleware', 'type[MainMiddleware]']]]
    context: NotRequired['MainContext']
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