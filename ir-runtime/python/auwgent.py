"""
Auwgent Runtime — Type-Safe Python Wrapper

This module provides a type-safe layer on top of the native Rust binding (PyO3).
It mirrors the TypeScript SDK's feature set: deferred listeners, middleware pipeline,
object-style intent handlers, and proper session lifecycle management.
"""

import json
import sys
from typing import (
    Any, Callable, Awaitable, Dict, List, Optional,
    TypeVar, Generic, Protocol, Union, TypedDict,
)

# NotRequired is 3.11+; fall back to typing_extensions for 3.9/3.10
try:
    from typing import NotRequired
except ImportError:
    from typing_extensions import NotRequired

try:
    from . import auwgent_native
except ImportError:
    import auwgent_native

# ── Type Variables ────────────────────────────────────────────────────────
AgentIR = TypeVar("AgentIR")
AgentContext = TypeVar("AgentContext")
AgentOutput = TypeVar("AgentOutput")
AgentTools = TypeVar("AgentTools")

# ── Session State ─────────────────────────────────────────────────────────

class SessionTurn(TypedDict, total=False):
    input: str
    model_response: str

class SessionState(TypedDict, total=False):
    """Exported session state from the engine."""
    systemPrompt: Optional[str]
    turns: List[SessionTurn]

# ── Error Types ───────────────────────────────────────────────────────────

class AuwgentToolError(Exception):
    """Raised when a tool execution fails inside the engine."""

    def __init__(self, tool: str, message: str):
        self.tool = tool
        self.message = message
        super().__init__(f"Tool '{tool}' failed: {message}")

# ── Middleware Types ──────────────────────────────────────────────────────

class MiddlewareContext(TypedDict, total=False):
    activeAgent: str
    stack: List[str]
    rootAgent: str
    rawBlock: Optional[str]
    systemPrompt: Optional[str]

class Middleware(Protocol):
    name: str
    target: NotRequired[Union[str, List[str]]]

    async def onRunStart(self, session: Dict[str, Any], ctx: MiddlewareContext) -> Dict[str, Any]: ...
    async def onLLMStart(self, prompt: str, ctx: MiddlewareContext) -> Optional[str]: ...
    async def onIntent(self, name: str, value: Dict[str, Any], ctx: MiddlewareContext) -> Optional[Dict[str, Any]]: ...
    async def onLLMEnd(self, response: Dict[str, Any], ctx: MiddlewareContext) -> None: ...
    async def onRunComplete(self, finalSession: Dict[str, Any], ctx: MiddlewareContext) -> None: ...
    async def onError(self, error: Exception, session: Optional[Dict[str, Any]], ctx: MiddlewareContext) -> bool: ...

# ── Type-Safe Auwgent Wrapper ─────────────────────────────────────────────

class TypedAuwgent(Generic[AgentIR, AgentContext, AgentOutput, AgentTools]):
    """
    Production-grade, type-safe wrapper around the Native Rust Engine (PyO3).
    
    Mirrors the TypeScript SDK's full feature set:
    - Deferred listener registration (activate/deactivate around each run)
    - Middleware pipeline with onIntent, onLLMStart/End, onRunStart/Complete
    - Object-style intent handlers via on_handlers() / on_handlers_partial()
    - Session import/export for persistence
    """

    def __init__(self, ir_json: str):
        self._native = auwgent_native.AuwgentNative(ir_json)
        self.ir: Dict[str, Any] = json.loads(ir_json)
        self.middleware: List[Any] = []
        self._shared_context: Dict[str, Any] = {}
        self._agent_stack: List[str] = []

        # Deferred handler storage — registered/deregistered around each run()
        self._stored_intent_handler: Optional[Callable] = None
        self._stored_partial_handler: Optional[Callable] = None
        self._last_intent_val: Any = None
        self._last_intent_name: Optional[str] = None

    # ── Driver Configuration ──────────────────────────────────────────────

    def set_gemini_driver(self, api_key: str) -> None:
        self._native.set_gemini_driver(api_key)

    def set_openai_driver(self, api_key: str, base_url: Optional[str] = None) -> None:
        self._native.set_openai_driver(api_key, base_url)

    def set_context(self, context: Dict[str, Any]) -> None:
        self._native.set_context(json.dumps(context))

    # ── Tool Registration ─────────────────────────────────────────────────

    def register_tool(self, name: str, callback: Callable[..., Awaitable[Any]]) -> None:
        async def wrap_callback(args_json_str: str) -> str:
            args_dict = json.loads(args_json_str)
            if not isinstance(args_dict, dict):
                args_dict = {}
            res = await callback(**args_dict)
            return json.dumps(res)
        self._native.register_tool(name, wrap_callback)

    # ── Intent Handlers ───────────────────────────────────────────────────

    def on_intent(self, callback: Callable[[str, Dict[str, Any]], Awaitable[Optional[Dict[str, Any]]]]) -> None:
        """
        Register an intent callback for real-time streaming events.
        The handler is stored and automatically registered/deregistered
        around each run() call.
        """
        self._stored_intent_handler = callback

    def on_intent_partial(self, callback: Callable[[str, Dict[str, Any]], None]) -> None:
        """
        Register a partial intent callback for streaming updates.
        Fires as YAML data streams in, BEFORE the intent block is complete.
        """
        self._stored_partial_handler = callback

    def on_handlers(self, handlers: Dict[str, Callable[[Dict[str, Any]], Awaitable[Any]]]) -> None:
        """
        Register multiple intent handlers using an object-style API.
        
        Example:
            agent.on_handlers({
                "response_text": lambda value: print(value.get("text")),
                "tool_call": lambda value: print(f"Calling {value.get('type')}"),
            })
        """
        async def dispatch(name: str, value: Dict[str, Any]) -> Optional[Dict[str, Any]]:
            handler = handlers.get(name)
            if handler:
                return await handler(value)
            return None
        self._stored_intent_handler = dispatch

    def on_handlers_partial(self, handlers: Dict[str, Callable[[Dict[str, Any]], None]]) -> None:
        """
        Register multiple partial intent handlers using an object-style API.
        
        Example:
            agent.on_handlers_partial({
                "response_text": lambda value: sys.stdout.write(value.get("text", "")),
            })
        """
        def dispatch(name: str, value: Dict[str, Any]) -> None:
            handler = handlers.get(name)
            if handler:
                handler(value)
        self._stored_partial_handler = dispatch

    # ── Middleware Helpers ─────────────────────────────────────────────────

    def _build_context(self) -> MiddlewareContext:
        active = self._agent_stack[-1] if self._agent_stack else self.ir.get("name", "agent")
        ctx = MiddlewareContext(
            activeAgent=active,
            stack=list(self._agent_stack),
            rootAgent=self.ir.get("name", "agent")
        )
        for k, v in self._shared_context.items():
            ctx[k] = v  # type: ignore
        return ctx

    def _get_middleware(self, ctx: MiddlewareContext) -> List[Any]:
        valid: List[Any] = []
        for m in self.middleware:
            target = getattr(m, "target", None)
            if not target:
                valid.append(m)
                continue
            targets = target if isinstance(target, list) else [target]
            if ctx["activeAgent"] in targets:
                valid.append(m)
        return valid

    # ── Listener Lifecycle ────────────────────────────────────────────────

    def _activate_listeners(self) -> None:
        """
        Bind the stored handlers (including middleware pipeline) to the Rust engine.
        After run() completes, _deactivate_listeners() replaces them with no-ops.
        """
        # ── 1. Intent Interceptor ──
        user_handler = self._stored_intent_handler

        async def wrap_intent(name: str, value_json_str: str) -> Optional[str]:
            val_dict: Dict[str, Any] = json.loads(value_json_str)
            ctx = self._build_context()

            # Extract _raw from Rust-injected field
            raw_block = val_dict.pop("_raw", None)
            if raw_block is not None:
                ctx["rawBlock"] = raw_block

            # Pipeline through middleware
            for m in self._get_middleware(ctx):
                if hasattr(m, "onIntent"):
                    try:
                        control = await m.onIntent(name, val_dict, ctx)
                        if control is not None:
                            return json.dumps(control)
                    except Exception:
                        raise

            # Track last intent value for onLLMEnd
            self._last_intent_name = name
            self._last_intent_val = val_dict

            # Forward to user handler
            if user_handler:
                res = await user_handler(name, val_dict)

                # Unified Error Hook Bridge
                if name == "tool_error" and isinstance(val_dict, dict):
                    tool_err = AuwgentToolError(
                        val_dict.get("tool", "unknown"),
                        val_dict.get("message", "unknown"),
                    )
                    for m in self._get_middleware(ctx):
                        if hasattr(m, "onError"):
                            await m.onError(tool_err, None, ctx)

                return json.dumps(res) if res is not None else None

            # Error hook bridge even without user handler
            if name == "tool_error" and isinstance(val_dict, dict):
                tool_err = AuwgentToolError(
                    val_dict.get("tool", "unknown"),
                    val_dict.get("message", "unknown"),
                )
                for m in self._get_middleware(ctx):
                    if hasattr(m, "onError"):
                        await m.onError(tool_err, None, ctx)

            return None

        self._native.on_intent(wrap_intent)

        # ── 2. Partial Intent Interceptor ──
        def wrap_partial(name: str, value_json_str: str) -> None:
            if self._stored_partial_handler:
                self._stored_partial_handler(name, json.loads(value_json_str))
        self._native.on_intent_partial(wrap_partial)

        # ── 3. SubEngine Hooks ──
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

        # ── 4. LLM Hooks ──
        async def wrap_llm_start(prompt_json: str, system_prompt: str) -> Optional[str]:
            ctx = self._build_context()
            ctx["systemPrompt"] = system_prompt
            self._last_intent_name = None
            self._last_intent_val = None

            current_prompt = prompt_json
            modified = False
            for m in self._get_middleware(ctx):
                if hasattr(m, "onLLMStart"):
                    result = await m.onLLMStart(current_prompt, ctx)
                    if isinstance(result, str):
                        current_prompt = result
                        modified = True

            # Only return the string if middleware actually modified it.
            # Returning None tells the Rust engine "no modification".
            return current_prompt if modified else None
        self._native.on_llm_start(wrap_llm_start)

        async def wrap_llm_end(response_str: str, system_prompt: str) -> None:
            ctx = self._build_context()
            ctx["systemPrompt"] = system_prompt

            if self._last_intent_name in ("response_text", "response_schema"):
                for m in self._get_middleware(ctx):
                    if hasattr(m, "onLLMEnd"):
                        await m.onLLMEnd(self._last_intent_val or {}, ctx)
        self._native.on_llm_end(wrap_llm_end)

    def _deactivate_listeners(self) -> None:
        """Replace native listeners with no-ops so the engine can exit cleanly."""
        async def noop_intent(a: str, b: str) -> Optional[str]:
            return None

        def noop_partial(a: str, b: str) -> None:
            pass

        async def noop_sub_start(a: str, b: str) -> Optional[str]:
            return None

        async def noop_sub_complete(a: str, b: str) -> None:
            pass

        async def noop_llm_start(a: str, b: str) -> Optional[str]:
            return None

        async def noop_llm_end(a: str, b: str) -> None:
            pass

        self._native.on_intent(noop_intent)
        self._native.on_intent_partial(noop_partial)
        self._native.on_sub_engine_start(noop_sub_start)
        self._native.on_sub_engine_complete(noop_sub_complete)
        self._native.on_llm_start(noop_llm_start)
        self._native.on_llm_end(noop_llm_end)

    # ── Execution ─────────────────────────────────────────────────────────

    async def run(self, input_val: Any = None) -> SessionState:
        """Run the agentic loop. Returns the exported session state."""
        self._shared_context = {}
        self._agent_stack = [self.ir.get("name", "agent")]

        current_session = self.export_session()
        self._activate_listeners()
        ctx = self._build_context()

        try:
            # onRunStart interception
            for m in self._get_middleware(ctx):
                if hasattr(m, "onRunStart"):
                    current_session = await m.onRunStart(current_session, ctx)

            self.import_session(current_session)

            # Pass raw strings directly; only JSON-encode non-string types
            if input_val is None:
                input_str = None
            elif isinstance(input_val, str):
                input_str = input_val
            else:
                input_str = json.dumps(input_val)

            res_json = await self._native.run(input_str)
            current_session = json.loads(res_json)

            # onRunComplete interception
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
                raise
            return current_session
        finally:
            self._deactivate_listeners()

    # ── Session Management ────────────────────────────────────────────────

    def export_session(self) -> SessionState:
        """Export session state for persistence."""
        return json.loads(self._native.export_session())

    def import_session(self, session: Any) -> None:
        """Import a previously exported session state."""
        if isinstance(session, str):
            self._native.import_session(session)
        else:
            self._native.import_session(json.dumps(session))

    def clear_session(self) -> None:
        """Clear the session (fresh conversation)."""
        self._native.clear_session()

    # ── Introspection ─────────────────────────────────────────────────────

    def generate_prompt(self) -> str:
        """Generate the system prompt (debugging)."""
        return self._native.generate_prompt()

    def get_tool_names(self) -> List[str]:
        """Get tool names defined in the IR."""
        return self._native.get_tool_names()

    def get_tool_schemas(self) -> List[Dict[str, Any]]:
        """Get tool schemas (for introspection)."""
        return json.loads(self._native.get_tool_schemas())

    @property
    def raw(self) -> Any:
        """Access the raw native binding for advanced usage."""
        return self._native

    # ── Streaming Primitives (advanced) ───────────────────────────────────

    async def process_intents(self) -> Dict[str, Any]:
        res_json = await self._native.process_intents()
        return json.loads(res_json)

    def end_stream(self) -> str:
        return self._native.end_stream()

    def write_chunk(self, chunk: str) -> None:
        self._native.write_chunk(chunk)


# ── Factory Function ──────────────────────────────────────────────────────

def create_auwgent(ir_dict: Dict[str, Any], config: Any) -> TypedAuwgent:
    """
    Factory function to create a fully configured TypedAuwgent.
    Equivalent to the generated create[AgentName]() in TypeScript.
    """
    ir_str = json.dumps(ir_dict)
    agent: TypedAuwgent = TypedAuwgent(ir_str)

    # Middleware
    middlewares = config.get("middleware", []) if isinstance(config, dict) else getattr(config, "middleware", [])
    if middlewares:
        agent.middleware = list(middlewares)

    # Tools — supports both dict-style and class-style
    tools = config.get("tools", None) if isinstance(config, dict) else getattr(config, "tools", None)
    if tools is not None:
        if isinstance(tools, dict):
            for tool_name, tool_func in tools.items():
                if callable(tool_func):
                    agent.register_tool(tool_name, tool_func)
        else:
            # Class-style: iterate tool names from IR and bind methods
            for tool_name in agent.get_tool_names():
                if hasattr(tools, tool_name):
                    tool_func = getattr(tools, tool_name)
                    if callable(tool_func):
                        agent.register_tool(tool_name, tool_func)

    # Context
    context = config.get("context") if isinstance(config, dict) else getattr(config, "context", None)
    if context:
        agent.set_context(context)

    # API Keys
    api_keys = config.get("apiKeys", {}) if isinstance(config, dict) else getattr(config, "apiKeys", {})
    if isinstance(api_keys, dict):
        if "geminiApiKey" in api_keys:
            agent.set_gemini_driver(api_keys["geminiApiKey"])
        if "openaiApiKey" in api_keys:
            agent.set_openai_driver(
                api_keys["openaiApiKey"],
                api_keys.get("customUrl"),
            )

    return agent
