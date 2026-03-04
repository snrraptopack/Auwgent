import json
from typing import Any, Callable, Awaitable, Dict, List, Optional, TypeVar, Generic, Protocol, NotRequired, Union, TypedDict

try:
    from . import auwgent_native
except ImportError:
    import auwgent_native

AgentIR = TypeVar("AgentIR")
AgentContext = TypeVar("AgentContext")
AgentOutput = TypeVar("AgentOutput")
AgentTools = TypeVar("AgentTools")

class MiddlewareContext(TypedDict, total=False):
    activeAgent: str
    stack: List[str]
    rootAgent: str
    rawBlock: Optional[str]
    systemPrompt: Optional[str]
    # Allow custom middleware keys
    # Arbitrary k/v are supported via dict expansion in typed dicts starting 3.11, or just let users duck-type.

class Middleware(Protocol):
    name: str
    target: NotRequired[Union[str, List[str]]]

    async def onRunStart(self, session: Dict[str, Any], ctx: MiddlewareContext) -> Dict[str, Any]: ...
    async def onLLMStart(self, prompt: str, ctx: MiddlewareContext) -> Optional[str]: ...
    async def onIntent(self, name: str, value: Dict[str, Any], ctx: MiddlewareContext) -> Optional[Dict[str, Any]]: ...
    async def onLLMEnd(self, response: Dict[str, Any], ctx: MiddlewareContext) -> None: ...
    async def onRunComplete(self, finalSession: Dict[str, Any], ctx: MiddlewareContext) -> None: ...
    async def onError(self, error: Exception, session: Optional[Dict[str, Any]], ctx: MiddlewareContext) -> bool: ...

class TypedAuwgent(Generic[AgentIR, AgentContext, AgentOutput, AgentTools]):
    """
    A strong-typed wrapper around the Native Rust Engine using PyO3.
    """
    def __init__(self, ir_json: str):
        self._native = auwgent_native.AuwgentNative(ir_json)
        self.ir = json.loads(ir_json)
        self.middleware: List[Any] = []
        self._shared_context: Dict[str, Any] = {}
        self._agent_stack: List[str] = []
        
        self._stored_intent_handler: Optional[Callable] = None
        self._stored_partial_handler: Optional[Callable] = None
        self._last_intent_val: Any = None
        self._last_intent_name: Optional[str] = None

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
        self._stored_intent_handler = callback

    def on_intent_partial(self, callback: Callable[[str, Dict[str, Any]], None]):
        self._stored_partial_handler = callback
        
    def _build_context(self) -> MiddlewareContext:
        active = self._agent_stack[-1] if self._agent_stack else self.ir.get("name", "agent")
        ctx = MiddlewareContext(
            activeAgent=active,
            stack=list(self._agent_stack),
            rootAgent=self.ir.get("name", "agent")
        )
        # Mix in any shared state
        for k, v in self._shared_context.items():
            ctx[k] = v  # type: ignore
        return ctx

    def _get_middleware(self, ctx: MiddlewareContext) -> List[Any]:
        valid = []
        for m in self.middleware:
            target = getattr(m, "target", None)
            if not target:
                valid.append(m)
                continue
            targets = target if isinstance(target, list) else [target]
            if ctx["activeAgent"] in targets:
                valid.append(m)
        return valid

    def _activate_listeners(self):
        # 1. Intent Interceptor
        async def wrap_intent(name: str, value_json_str: str) -> Optional[str]:
            val_dict = json.loads(value_json_str)
            ctx = self._build_context()
            
            raw_block = val_dict.pop("_raw", None)
            if raw_block:
                ctx["rawBlock"] = raw_block
                
            for m in self._get_middleware(ctx):
                if hasattr(m, "onIntent"):
                    control = await m.onIntent(name, val_dict, ctx)
                    if control is not None:
                        return json.dumps(control)
            
            self._last_intent_name = name
            self._last_intent_val = val_dict
            
            if self._stored_intent_handler:
                res = await self._stored_intent_handler(name, val_dict)
                return json.dumps(res) if res is not None else None
                
            return None
            
        self._native.on_intent(wrap_intent)

        # 2. Partial Intent Interceptor
        def wrap_partial(name: str, value_json_str: str) -> None:
            if self._stored_partial_handler:
                self._stored_partial_handler(name, json.loads(value_json_str))
        self._native.on_intent_partial(wrap_partial)
        
        # 3. SubEngine Hooks
        async def wrap_sub_start(helper_name: str, session_json: str) -> Optional[str]:
            session = json.loads(session_json)
            self._agent_stack.append(helper_name)
            ctx = self._build_context()
            ctx["systemPrompt"] = session.get("systemPrompt")
            
            for m in self._get_middleware(ctx):
                if hasattr(m, "onRunStart"):
                    session = await m.onRunStart(session, ctx)
            return json.dumps(session)
        self._native.on_sub_engine_start(wrap_sub_start)
        
        async def wrap_sub_complete(helper_name: str, session_json: str) -> None:
            session = json.loads(session_json)
            ctx = self._build_context()
            ctx["systemPrompt"] = session.get("systemPrompt")
            
            for m in self._get_middleware(ctx):
                if hasattr(m, "onRunComplete"):
                    await m.onRunComplete(session, ctx)
            if self._agent_stack:
                self._agent_stack.pop()
        self._native.on_sub_engine_complete(wrap_sub_complete)
        
        # 4. LLM Hooks
        async def wrap_llm_start(prompt_json: str, system_prompt: str) -> Optional[str]:
            ctx = self._build_context()
            ctx["systemPrompt"] = system_prompt
            self._last_intent_name = None
            self._last_intent_val = None
            
            current_prompt = prompt_json
            for m in self._get_middleware(ctx):
                if hasattr(m, "onLLMStart"):
                    modified = await m.onLLMStart(current_prompt, ctx)
                    if isinstance(modified, str):
                        current_prompt = modified
            return current_prompt
        self._native.on_llm_start(wrap_llm_start)
        
        async def wrap_llm_end(response_str: str, system_prompt: str) -> None:
            ctx = self._build_context()
            ctx["systemPrompt"] = system_prompt
            
            if self._last_intent_name in ("response_text", "response_schema"):
                for m in self._get_middleware(ctx):
                    if hasattr(m, "onLLMEnd"):
                        await m.onLLMEnd(self._last_intent_val or {}, ctx)
        self._native.on_llm_end(wrap_llm_end)

    def _deactivate_listeners(self):
        async def noop_str_str_opt(a: str, b: str) -> Optional[str]: return None
        async def noop_str_str(a: str, b: str) -> None: return None
        def noop_str_str_sync(a: str, b: str) -> None: return None
        
        self._native.on_intent(noop_str_str_opt)
        self._native.on_intent_partial(noop_str_str_sync)
        self._native.on_sub_engine_start(noop_str_str_opt)
        self._native.on_sub_engine_complete(noop_str_str)
        self._native.on_llm_start(noop_str_str_opt)
        self._native.on_llm_end(noop_str_str)

    async def run(self, input_val: Any = None) -> AgentOutput:
        self._shared_context = {}
        self._agent_stack = [self.ir.get("name", "agent")]
        
        current_session = json.loads(self.export_session())
        self._activate_listeners()
        ctx = self._build_context()
        
        try:
            for m in self._get_middleware(ctx):
                if hasattr(m, "onRunStart"):
                    current_session = await m.onRunStart(current_session, ctx)
                    
            self.import_session(json.dumps(current_session))
            input_str = json.dumps(input_val) if input_val is not None else None
            res_json = await self._native.run(input_str)
            current_session = json.loads(res_json)
            
            for m in self._get_middleware(ctx):
                if hasattr(m, "onRunComplete"):
                    await m.onRunComplete(current_session, ctx)
                    
            return current_session
            
        except Exception as e:
            handled = False
            for m in self._get_middleware(ctx):
                if hasattr(m, "onError"):
                    swallow = await m.onError(e, current_session, ctx)
                    if swallow:
                        handled = True
                        break
            if not handled:
                raise e
            return current_session
        finally:
            self._deactivate_listeners()
        
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
    
    middlewares = config.get("middleware", [])
    if middlewares:
        agent.middleware = middlewares
        
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
