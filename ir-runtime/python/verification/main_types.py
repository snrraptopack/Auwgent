# Auto-generated types for Manager
# Do not edit manually
import os
import json
from typing import TypedDict, Callable, Awaitable, Any, List, Dict, Union, Optional, Protocol
# NotRequired is 3.11+; fall back to typing_extensions for 3.9/3.10
try:
    from typing import NotRequired
except ImportError:
    from typing_extensions import NotRequired
try:
    from auwgent import TypedAuwgent, create_auwgent, Middleware, MiddlewareContext, SessionState, AuwgentToolError
except ImportError:
    # For local testing if auwgent is not installed via pip
    import sys
    sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))
    from auwgent import TypedAuwgent, create_auwgent, Middleware, MiddlewareContext, SessionState, AuwgentToolError
class Student(TypedDict, total=False):
    user_name: str
    age: float
    id: str
    grades: List[str]

class ManagerInput(TypedDict, total=False):
    pass

class ManagerOutput(TypedDict, total=False):
    pass

class ManagerContext(TypedDict, total=False):
    user_name: str

class ManagerTools(TypedDict, total=False):
    # This is used to get the details of the student
    getStudentDetails: Callable[[str], Awaitable["Student"]]

class ManagerApiKeys(TypedDict, total=False):
    geminiApiKey: str

ManagerAgent = TypedAuwgent

ManagerMiddleware = Middleware

class ManagerConfig(TypedDict, total=False):
    tools: NotRequired['ManagerTools']
    middleware: NotRequired[List['ManagerMiddleware']]
    context: NotRequired['ManagerContext']
    apiKeys: NotRequired['ManagerApiKeys']

def createManager(config: ManagerConfig) -> 'ManagerAgent':
    """Create a fully configured Manager agent from config."""
    ir_path = os.path.join(os.path.dirname(__file__), "main.agent.json")
    with open(ir_path, "r", encoding="utf-8") as f:
        ir_dict = json.load(f)
    return create_auwgent(ir_dict, config)

auwgent = createManager
AuwgentTools = ManagerTools
AuwgentConfig = ManagerConfig
AuwgentAgent = ManagerAgent
AuwgentMiddleware = ManagerMiddleware
AuwgentContext = ManagerContext
