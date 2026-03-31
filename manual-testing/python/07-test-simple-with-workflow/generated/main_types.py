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
    user_id: str
    quantity: float
    product_id: str
    status: str
    total: float
    id: str

class QueryResult(TypedDict, total=False):
    data: str
    message: str
    success: bool

class User(TypedDict, total=False):
    name: str
    created_at: str
    id: str
    email: str

class AnalysisReport(TypedDict, total=False):
    total_users: float
    total_products: float
    total_orders: float
    insights: List[str]
    revenue: float

class Product(TypedDict, total=False):
    name: str
    stock: float
    id: str
    price: float
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

class MainTools(TypedDict, total=False):
    # Query users from in-memory DB. Filter can be 'all', 'id:<id>', 'email:<email>'
    db_query_users: Callable[[str], Awaitable[List["User"]]]

    # Query products from in-memory DB. Filter can be 'all', 'id:<id>', 'name:<name>'
    db_query_products: Callable[[str], Awaitable[List["Product"]]]

    # Query orders from in-memory DB. Filter can be 'all', 'user_id:<id>', 'status:<status>'
    db_query_orders: Callable[[str], Awaitable[List["Order"]]]

    # Create a new user in the database
    db_create_user: Callable[[str, str], Awaitable["User"]]

    # Create a new product in the database
    db_create_product: Callable[[str, float, float], Awaitable["Product"]]

    # Create a new order in the database
    db_create_order: Callable[[str, str, float], Awaitable["Order"]]

    # Parse orders JSON and sum totals
    sum_order_totals: Callable[[str], Awaitable[float]]

    # Check if enough stock available
    validate_stock: Callable[[str, float], Awaitable[bool]]

    # Parse comma-separated values
    parse_csv: Callable[[str], Awaitable[str]]

    # Analyze a specific user's purchasing behavior
    analyze_user_behavior: Callable[[str], Awaitable[str]]

    # Find products with low stock levels
    detect_low_stock: Callable[[], Awaitable[str]]

    # Calculate average from comma-separated numbers
    calculate_average: Callable[[str], Awaitable[float]]

    # Identify outliers in dataset
    find_outliers: Callable[[str], Awaitable[str]]

    # Format data as a text table
    format_table: Callable[[str], Awaitable[str]]

    # Generate a textual description of a chart
    generate_chart_description: Callable[[str, str], Awaitable[str]]

    # Group orders by status and count
    aggregate_by_status: Callable[[str], Awaitable[str]]

    # Calculate key sales metrics
    calculate_metrics: Callable[[str], Awaitable[str]]

MainCustomIntents = TypedDict('_SpeakLoudCustomIntent', {"name": Literal["SpeakLoud"], "value": {"explain": str}}, total=False)

class MainResponseTextIntent(TypedDict, total=False):
    text: str

MainResponseSchemaIntent = MainOutput

class MainErrorIntent(TypedDict, total=False):
    message: str
class Mainget_user_ordersWorkflowArgs(TypedDict, total=False):
    target_user_id: str

Mainget_user_ordersWorkflowResultValue = str

class Mainget_user_ordersWorkflowCall(TypedDict, total=False):
    type: Literal["get_user_orders"]
    args: Mainget_user_ordersWorkflowArgs

class Mainget_user_ordersWorkflowResult(TypedDict, total=False):
    name: Literal["get_user_orders"]
    result: Mainget_user_ordersWorkflowResultValue

class Maincalculate_revenueWorkflowArgs(TypedDict, total=False):
    pass

Maincalculate_revenueWorkflowResultValue = str

class Maincalculate_revenueWorkflowCall(TypedDict, total=False):
    type: Literal["calculate_revenue"]
    args: Maincalculate_revenueWorkflowArgs

class Maincalculate_revenueWorkflowResult(TypedDict, total=False):
    name: Literal["calculate_revenue"]
    result: Maincalculate_revenueWorkflowResultValue

class Mainprocess_bulk_orderWorkflowArgs(TypedDict, total=False):
    target_user_id: str
    product_ids: str
    quantities: str

Mainprocess_bulk_orderWorkflowResultValue = str

class Mainprocess_bulk_orderWorkflowCall(TypedDict, total=False):
    type: Literal["process_bulk_order"]
    args: Mainprocess_bulk_orderWorkflowArgs

class Mainprocess_bulk_orderWorkflowResult(TypedDict, total=False):
    name: Literal["process_bulk_order"]
    result: Mainprocess_bulk_orderWorkflowResultValue

MainDataAnalyzerHelperArgs = Dict[str, Any]

class MainDataAnalyzerHelperResultValue(TypedDict, total=False):
    total_users: float
    total_products: float
    total_orders: float
    revenue: float
    insights: List[str]

class MainDataAnalyzerHelperCall(TypedDict, total=False):
    type: Literal["DataAnalyzer"]
    args: MainDataAnalyzerHelperArgs

class MainDataAnalyzerHelperResult(TypedDict, total=False):
    name: Literal["DataAnalyzer"]
    result: MainDataAnalyzerHelperResultValue

MainReportGeneratorHelperArgs = Dict[str, Any]

MainReportGeneratorHelperResultValue = Dict[str, Any]

class MainReportGeneratorHelperCall(TypedDict, total=False):
    type: Literal["ReportGenerator"]
    args: MainReportGeneratorHelperArgs

class MainReportGeneratorHelperResult(TypedDict, total=False):
    name: Literal["ReportGenerator"]
    result: MainReportGeneratorHelperResultValue

MainIntentValue = Union[
    MainResponseTextIntent,
    MainResponseSchemaIntent,
    MainErrorIntent,
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
MainIntentName = Literal["response_text", "response_schema", "error", "workflow_call", "workflow_result", "helper_call", "helper_result"]

MainIntentHandler = Callable[[MainIntentName, MainIntentValue, str], Awaitable[Optional[Dict[str, Any]]]]
MainPartialIntentHandler = Callable[[MainIntentName, MainIntentValue, str], None]

class MainIntentHandlers(TypedDict, total=False):
    response_text: Callable[[MainResponseTextIntent], Awaitable[Any]]
    response_schema: Callable[[MainResponseSchemaIntent], Awaitable[Any]]
    error: Callable[[MainErrorIntent], Awaitable[Any]]
    workflow_call: Callable[[Union[Mainget_user_ordersWorkflowCall, Maincalculate_revenueWorkflowCall, Mainprocess_bulk_orderWorkflowCall]], Awaitable[Any]]
    workflow_result: Callable[[Union[Mainget_user_ordersWorkflowResult, Maincalculate_revenueWorkflowResult, Mainprocess_bulk_orderWorkflowResult]], Awaitable[Any]]
    helper_call: Callable[[Union[MainDataAnalyzerHelperCall, MainReportGeneratorHelperCall]], Awaitable[Any]]
    helper_result: Callable[[Union[MainDataAnalyzerHelperResult, MainReportGeneratorHelperResult]], Awaitable[Any]]

class MainPartialIntentHandlers(TypedDict, total=False):
    response_text: Callable[[MainResponseTextIntent], None]
    response_schema: Callable[[MainResponseSchemaIntent], None]
    error: Callable[[MainErrorIntent], None]
    workflow_call: Callable[[Union[Mainget_user_ordersWorkflowCall, Maincalculate_revenueWorkflowCall, Mainprocess_bulk_orderWorkflowCall]], None]
    workflow_result: Callable[[Union[Mainget_user_ordersWorkflowResult, Maincalculate_revenueWorkflowResult, Mainprocess_bulk_orderWorkflowResult]], None]
    helper_call: Callable[[Union[MainDataAnalyzerHelperCall, MainReportGeneratorHelperCall]], None]
    helper_result: Callable[[Union[MainDataAnalyzerHelperResult, MainReportGeneratorHelperResult]], None]

class MainApiKeys(TypedDict, total=False):
    my_groq_apiApiKey: str  # API key for custom provider 'my-groq-api'

class MainAgent(TypedAuwgent[Any, MainContext, MainOutput, MainTools]):
    @overload
    def on_intent(self, callback: Callable[[Literal["response_text"], MainResponseTextIntent, str], Awaitable[Optional[Dict[str, Any]]]]) -> None: ...
    @overload
    def on_intent(self, callback: Callable[[Literal["response_schema"], MainResponseSchemaIntent, str], Awaitable[Optional[Dict[str, Any]]]]) -> None: ...
    @overload
    def on_intent(self, callback: Callable[[Literal["error"], MainErrorIntent, str], Awaitable[Optional[Dict[str, Any]]]]) -> None: ...
    @overload
    def on_intent(self, callback: Callable[[Literal["workflow_call"], MainWorkflowCallIntentValue, str], Awaitable[Optional[Dict[str, Any]]]]) -> None: ...
    @overload
    def on_intent(self, callback: Callable[[Literal["workflow_result"], MainWorkflowResultIntentValue, str], Awaitable[Optional[Dict[str, Any]]]]) -> None: ...
    @overload
    def on_intent(self, callback: Callable[[Literal["helper_call"], MainHelperCallIntentValue, str], Awaitable[Optional[Dict[str, Any]]]]) -> None: ...
    @overload
    def on_intent(self, callback: Callable[[Literal["helper_result"], MainHelperResultIntentValue, str], Awaitable[Optional[Dict[str, Any]]]]) -> None: ...
    def on_intent(self, callback: MainIntentHandler) -> None:
        return super().on_intent(callback)

    @overload
    def on_intent_partial(self, callback: Callable[[Literal["response_text"], MainResponseTextIntent, str], None]) -> None: ...
    @overload
    def on_intent_partial(self, callback: Callable[[Literal["response_schema"], MainResponseSchemaIntent, str], None]) -> None: ...
    @overload
    def on_intent_partial(self, callback: Callable[[Literal["error"], MainErrorIntent, str], None]) -> None: ...
    @overload
    def on_intent_partial(self, callback: Callable[[Literal["workflow_call"], MainWorkflowCallIntentValue, str], None]) -> None: ...
    @overload
    def on_intent_partial(self, callback: Callable[[Literal["workflow_result"], MainWorkflowResultIntentValue, str], None]) -> None: ...
    @overload
    def on_intent_partial(self, callback: Callable[[Literal["helper_call"], MainHelperCallIntentValue, str], None]) -> None: ...
    @overload
    def on_intent_partial(self, callback: Callable[[Literal["helper_result"], MainHelperResultIntentValue, str], None]) -> None: ...
    def on_intent_partial(self, callback: MainPartialIntentHandler) -> None:
        return super().on_intent_partial(callback)

    def on_handlers(self, handlers: MainIntentHandlers) -> None:
        return super().on_handlers(handlers)

    def on_handlers_partial(self, handlers: MainPartialIntentHandlers) -> None:
        return super().on_handlers_partial(handlers)

MainMiddleware = Middleware

class MainConfig(TypedDict, total=False):
    tools: NotRequired['MainTools']
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
AuwgentIntentHandlers = MainIntentHandlers
AuwgentPartialIntentHandlers = MainPartialIntentHandlers