# Auto-generated types for Manager
# Do not edit manually
from typing import TypedDict, Callable, Awaitable, Any, List, Dict, Union, Optional, Protocol, NotRequired
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
