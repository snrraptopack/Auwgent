# Auto-generated types for Test
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

class A(TypedDict, total=False):
    wow: str

class Hey(TypedDict, total=False):
    name: str
    age: float
class TestInput(TypedDict, total=False):
    text: str

class TestOutput_Hey(TypedDict, total=False):
    name: str
    age: float

class TestOutput_A(TypedDict, total=False):
    wow: str

TestOutput = Union[TestOutput_Hey, TestOutput_A]

class TestContext(TypedDict, total=False):
    pass

class TestTools(TypedDict, total=False):
    pass

class TestApiKeys(TypedDict, total=False):
    openaiApiKey: str
    customUrl: NotRequired[str]  # type: ignore

TestAgent = TypedAuwgent

TestMiddleware = Middleware

class TestConfig(TypedDict, total=False):
    tools: NotRequired['TestTools']
    middleware: NotRequired[List['TestMiddleware']]
    apiKeys: NotRequired['TestApiKeys']

def createTest(config: TestConfig) -> 'TestAgent':
    """Create a fully configured Test agent from config."""
    ir_path = os.path.join(os.path.dirname(__file__), "main.agent.json")
    with open(ir_path, "r", encoding="utf-8") as f:
        ir_dict = json.load(f)
    return create_auwgent(ir_dict, config)

auwgent = createTest
AuwgentTools = TestTools
AuwgentConfig = TestConfig
AuwgentAgent = TestAgent
AuwgentMiddleware = TestMiddleware
AuwgentContext = TestContext