# Auto-generated types for Manager
# Do not edit manually
import os
import json
from typing import TypedDict, Callable, Awaitable, Any, List, Dict, Union, Optional, Protocol, NotRequired
try:
    from auwgent import TypedAuwgent, create_auwgent
except ImportError:
    # For local testing if auwgent is not installed via pip
    import sys
    sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))
    from auwgent import TypedAuwgent, create_auwgent
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

class ManagerTools(Protocol):
    def getStudentDetails(
        self,
        id: str
    ) -> Awaitable["Student"]:
        """This is used to get the details of the student"""
        ...

class ManagerApiKeys(TypedDict, total=False):
    geminiApiKey: str

class ManagerConfig(TypedDict, total=False):
    tools: NotRequired['ManagerTools']
    context: NotRequired['ManagerContext']
    apiKeys: NotRequired['ManagerApiKeys']

def createManager(config: ManagerConfig) -> TypedAuwgent:
    ir_path = os.path.join(os.path.dirname(__file__), "main.agent.json")
    with open(ir_path, "r", encoding="utf-8") as f:
        ir_dict = json.load(f)
    return create_auwgent(ir_dict, config)
