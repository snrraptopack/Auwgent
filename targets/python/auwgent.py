"""
Auwgent Runtime — Type-Safe Python Wrapper

This module provides a type-safe layer on top of the native Rust binding (PyO3).
It mirrors the TypeScript SDK's feature set: deferred listeners, middleware pipeline,
object-style intent handlers, and proper session lifecycle management.
"""

import json
import sys
import typing
from typing import (
    Any, Callable, Awaitable, Dict, List, Optional,
    TypeVar, Generic, Protocol, Union, TypedDict,
    cast
)

# NotRequired is 3.11+; fall back to typing_extensions for 3.9/3.10
try:
    from . import auwgent_sdk
except (ImportError, ValueError):
    try:
        import auwgent_sdk
    except ImportError:
        # Fallback for static analysis in some environments
        auwgent_sdk = Any 

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
    embed: Callable[[str], Awaitable[List[float]]]
    embedBatch: Callable[[List[str]], Awaitable[List[List[float]]]]
    set_context: Callable[[Dict[str, Any]], None]

class Middleware(Protocol):
    name: str
    target: Optional[Union[str, List[str]]] = None

    async def onRunStart(self, session: Dict[str, Any], ctx: MiddlewareContext) -> Dict[str, Any]: ...
    async def onLLMStart(self, prompt: str, ctx: MiddlewareContext) -> Optional[str]: ...
    async def onIntent(self, name: str, value: Dict[str, Any], ctx: MiddlewareContext) -> Optional[Dict[str, Any]]: ...
    async def onLLMEnd(self, response: Dict[str, Any], ctx: MiddlewareContext) -> None: ...
    async def onRunComplete(self, finalSession: Dict[str, Any], ctx: MiddlewareContext) -> None: ...
    async def onError(self, error: Exception, session: Optional[Dict[str, Any]], ctx: MiddlewareContext) -> bool: ...

# ── Configuration ────────────────────────────────────────────────────────

class AuwgentConfig(TypedDict, total=False):
    tools: Union[Dict[str, Callable[..., Awaitable[Any]]], Any]
    middleware: List[Any]
    context: Dict[str, Any]
    apiKeys: Dict[str, str]

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

    def __init__(self, ir: Any, config: AuwgentConfig):
        ir_json = json.dumps(ir) if isinstance(ir, dict) else ir
        self._native = auwgent_sdk.AuwgentNative(ir_json)
        self.ir: Dict[str, Any] = ir if isinstance(ir, dict) else json.loads(ir_json)
        self.middleware: List[Any] = [*cast(list, config.get("middleware", []))]
        self._shared_context: Dict[str, Any] = {}
        self._agent_stack: List[str] = []

        # Deferred handler storage — registered/deregistered around each run()
        self._stored_intent_handler: Optional[Callable] = None
        self._stored_partial_handler: Optional[Callable] = None
        self._last_intent_val: Any = None
        self._last_intent_name: Optional[str] = None
        self._last_raw_block: Optional[str] = None

        # ── 1. Context ──
        if "context" in config:
            self.set_context(config["context"])

        # ── 2. Drivers ──
        api_keys = cast(Dict[str, str], config.get("apiKeys", {}))
        if "geminiApiKey" in api_keys:
            self.set_gemini_driver(api_keys["geminiApiKey"])
        if "openaiApiKey" in api_keys:
            self.set_openai_driver(api_keys["openaiApiKey"])
        
        self._register_custom_drivers(api_keys)

        # ── 3. Tools ──
        tools = config.get("tools")
        if tools:
            if isinstance(tools, dict):
                for name, handler in tools.items():
                    self.register_tool(name, handler)
            else:
                # Class-style
                for name in self.get_tool_names():
                    if hasattr(tools, name):
                        handler = getattr(tools, name)
                        if callable(handler):
                            self.register_tool(name, cast(Callable[..., Awaitable[Any]], handler))

    def _register_custom_drivers(self, api_keys: Dict[str, str]):
        def collect_from_entry(entry: Dict[str, Any]):
            default_config = entry.get("defaultConfig")
            if default_config and isinstance(default_config, dict):
                model = default_config.get("model")
                if model and isinstance(model, dict) and model.get("type") == "custom":
                    id = model.get("id")
                    url = model.get("url")
                    if id and isinstance(id, str) and url:
                        key = api_keys.get(f"{id.replace('-', '_')}ApiKey")
                        if key:
                            self.set_custom_driver(id, key, url)
            
            named_configs = entry.get("namedConfig")
            if named_configs and isinstance(named_configs, list):
                for named in named_configs:
                    if isinstance(named, dict):
                        model = named.get("model")
                        if model and isinstance(model, dict) and model.get("type") == "custom":
                            id = model.get("id")
                            url = model.get("url")
                            if id and isinstance(id, str) and url:
                                key = api_keys.get(f"{id.replace('-', '_')}ApiKey")
                                if key:
                                    self.set_custom_driver(id, key, url)

        model_config = self.ir.get("modelConfig")
        if model_config:
            for entry in model_config:
                collect_from_entry(entry)
        
        helpers = self.ir.get("helpers")
        if helpers:
            for helper in helpers:
                helper_config = helper.get("modelConfig")
                if helper_config:
                    for entry in helper_config:
                        collect_from_entry(entry)

    # ── Driver Configuration ──────────────────────────────────────────────

    def set_gemini_driver(self, api_key: str) -> None:
        self._native.set_gemini_driver(api_key)

    def set_openai_driver(self, api_key: str, base_url: Optional[str] = None) -> None:
        self._native.set_openai_driver(api_key, base_url)

    def set_custom_driver(self, id: str, api_key: str, base_url: str) -> None:
        self._native.set_custom_driver(id, api_key, base_url)

    def set_context(self, context: Dict[str, Any]) -> None:
        self._native.set_context(json.dumps(context))

    async def embed(self, text: str) -> List[float]:
        """Generate an embedding for the given text using the configured model."""
        return await self._native.embed(text)

    async def embed_batch(self, texts: List[str]) -> List[List[float]]:
        """Generate embeddings for a batch of texts."""
        return await self._native.embed_batch(texts)

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
        """
        self._stored_intent_handler = callback

    def on_intent_partial(self, callback: Callable[[str, Dict[str, Any]], None]) -> None:
        """
        Register a partial intent callback for streaming updates.
        """
        self._stored_partial_handler = callback

    def on_handlers(self, handlers: Dict[str, Callable[[Dict[str, Any]], Awaitable[Any]]]) -> None:
        """
        Register multiple intent handlers using an object-style API.
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
            rootAgent=self.ir.get("name", "agent"),
            rawBlock=self._last_raw_block,
            embed=self.embed,
            embedBatch=self.embed_batch,
            set_context=self.set_context
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
        # ── 1. Intent Interceptor ──
        user_handler = self._stored_intent_handler

        async def wrap_intent(name: str, value_json_str: str) -> Optional[str]:
            val_dict: Dict[str, Any] = json.loads(value_json_str)
            ctx = self._build_context()

            # Extract _raw from Rust-injected field
            raw_block = val_dict.get("_raw")
            if raw_block is not None:
                ctx["rawBlock"] = raw_block
                self._last_raw_block = raw_block
                val_dict.pop("_raw", None)

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
        partial_handler = self._stored_partial_handler
        def wrap_partial(name: str, value_json_str: str) -> None:
            if partial_handler is not None:
                partial_handler(name, json.loads(value_json_str))
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
        self._native.clear_listeners()

    # ── Execution ─────────────────────────────────────────────────────────

    async def run(self, input_val: Any = None) -> SessionState:
        """Run the agentic loop. Returns the exported session state."""
        self._shared_context = {}
        self._agent_stack = [self.ir.get("name", "agent")]
        self._last_raw_block = None

        current_session = self.export_session()
        self._activate_listeners()
        ctx = self._build_context()

        try:
            # onRunStart Interception — supports Stack-Aware Resumption (Execution Tunneling)
            for m in self._get_middleware(ctx):
                if hasattr(m, "onRunStart"):
                    current_session = await m.onRunStart(current_session, ctx)

            self.import_session(current_session)

            # ── Execution Tunneling ──
            # Check if middleware mutated the stack
            injected_stack = ctx["stack"] if len(ctx["stack"]) > 1 else None

            # Pass raw strings directly; only JSON-encode non-string types
            if input_val is None:
                input_str = None
            elif isinstance(input_val, str):
                input_str = input_val
            else:
                input_str = json.dumps(input_val)

            res_json = await self._native.run(
                input_str, 
                json.dumps(injected_stack) if injected_stack else None
            )
            current_session = json.loads(res_json)

            # onRunComplete interception
            for m in self._get_middleware(ctx):
                if hasattr(m, "onRunComplete"):
                    await m.onRunComplete(current_session, ctx)

            return cast(SessionState, current_session)

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
            return cast(SessionState, current_session)
        finally:
            self._deactivate_listeners()

    # ── Session Management ────────────────────────────────────────────────

    def export_session(self) -> SessionState:
        """Export session state for persistence."""
        return cast(SessionState, json.loads(self._native.export_session()))

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


# ── Factory Functions ──────────────────────────────────────────────────────

def create_auwgent(ir_dict: Dict[str, Any], config: AuwgentConfig) -> TypedAuwgent:
    """
    Factory function to create a fully configured TypedAuwgent.
    Aligns with the TypeScript SDK's clean constructor pattern.
    """
    return TypedAuwgent(ir_dict, config)

def create_auwgent_from_ir_json(ir_json: str, config: AuwgentConfig) -> TypedAuwgent:
    """
    Create an agent from an IR JSON string.
    """
    return TypedAuwgent(ir_json, config)
