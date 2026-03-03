import json
from typing import Any, Callable, Awaitable, Dict, List, Optional, TypeVar, Generic, Protocol

try:
    from . import auwgent_native
except ImportError:
    import auwgent_native

AgentIR = TypeVar("AgentIR")
AgentContext = TypeVar("AgentContext")
AgentOutput = TypeVar("AgentOutput")
AgentTools = TypeVar("AgentTools")

class TypedAuwgent(Generic[AgentIR, AgentContext, AgentOutput, AgentTools]):
    """
    A strong-typed wrapper around the Native Rust Engine using PyO3.
    """
    def __init__(self, ir_json: str):
        self._native = auwgent_native.AuwgentNative(ir_json)

    def set_gemini_driver(self, api_key: str):
        self._native.set_gemini_driver(api_key)

    def set_openai_driver(self, api_key: str, base_url: Optional[str] = None):
        if base_url:
            self._native.set_openai_driver(api_key, base_url)
        else:
            self._native.set_openai_driver(api_key, None)

    def set_context(self, context: Dict[str, Any]):
        self._native.set_context(json.dumps(context))

    def register_tool(self, name: str, callback: Callable[..., Awaitable[Any]]):
        async def wrap_callback(args_json_str: str) -> str:
            args_dict = json.loads(args_json_str)
            res = await callback(**args_dict)
            return json.dumps(res)
        self._native.register_tool(name, wrap_callback)

    def on_intent(self, callback: Callable[[str, Dict[str, Any]], Awaitable[Optional[Dict[str, Any]]]]):
        async def wrap_callback(name: str, value_json_str: str) -> Optional[str]:
            val_dict = json.loads(value_json_str)
            res = await callback(name, val_dict)
            if res is not None:
                return json.dumps(res)
            return None
        self._native.on_intent(wrap_callback)
        
    def on_sub_engine_start(self, callback: Callable[[str, str], Awaitable[Optional[str]]]):
        self._native.on_sub_engine_start(callback)
        
    def on_sub_engine_complete(self, callback: Callable[[str, str], Awaitable[None]]):
        self._native.on_sub_engine_complete(callback)
        
    def on_llm_start(self, callback: Callable[[str, str], Awaitable[Optional[str]]]):
        self._native.on_llm_start(callback)
        
    def on_llm_end(self, callback: Callable[[str, str], Awaitable[None]]):
        self._native.on_llm_end(callback)
        
    def on_intent_partial(self, callback: Callable[[str, Dict[str, Any]], None]):
        def wrap_callback(name: str, value_json_str: str) -> None:
            val_dict = json.loads(value_json_str)
            callback(name, val_dict)
        self._native.on_intent_partial(wrap_callback)

    async def run(self, input_val: Any = None) -> AgentOutput:
        input_str = json.dumps(input_val) if input_val is not None else None
        res_json = await self._native.run(input_str)
        return json.loads(res_json)
        
    def get_tool_names(self) -> List[str]:
        return self._native.get_tool_names()
        
    def get_tool_schemas(self) -> str:
        return self._native.get_tool_schemas()
        
    def export_session(self) -> str:
        return self._native.export_session()
        
    def import_session(self, session_json: str):
        self._native.import_session(session_json)
        
    def clear_session(self):
        self._native.clear_session()
        
    def generate_prompt(self) -> str:
        return self._native.generate_prompt()

    async def process_intents(self) -> Dict[str, Any]:
        res_json = await self._native.process_intents()
        return json.loads(res_json)
        
    def end_stream(self) -> str:
        return self._native.end_stream()
        
    def write_chunk(self, chunk: str):
        self._native.write_chunk(chunk)


def create_auwgent(ir_dict: Dict[str, Any], config: Any) -> TypedAuwgent[Any, Any, Any, Any]:
    """
    Factory function to create a fully configured TypedAuwgent.
    Equivalent to the generated create[AgentName]() in TypeScript.
    """
    ir_str = json.dumps(ir_dict)
    agent = TypedAuwgent(ir_str)
    
    tools = config.get("tools", None)
    if tools is not None:
        if isinstance(tools, dict):
            for tool_name, tool_func in tools.items():
                if callable(tool_func):
                    agent.register_tool(tool_name, tool_func)
        else:
            for tool_name in agent.get_tool_names():
                if hasattr(tools, tool_name):
                    tool_func = getattr(tools, tool_name)
                    if callable(tool_func):
                        agent.register_tool(tool_name, tool_func)
            
    if "context" in config:
        agent.set_context(config["context"])
        
    api_keys = config.get("apiKeys", {})
    if "geminiApiKey" in api_keys:
        agent.set_gemini_driver(api_keys["geminiApiKey"])
    if "openaiApiKey" in api_keys:
        agent.set_openai_driver(api_keys["openaiApiKey"], api_keys.get("customUrl", None))
        
    return agent
