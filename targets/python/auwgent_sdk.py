"""
Auwgent Runtime — Type-Safe Python Wrapper

This module provides a type-safe layer on top of the native Rust binding (PyO3).
It mirrors the TypeScript SDK's feature set: deferred listeners, middleware pipeline,
object-style intent handlers, and proper session lifecycle management.
"""

import json
import inspect
import sys
import typing
from typing import (
    Any, Callable, Awaitable, Dict, List, Optional,
    TypeVar, Generic, Protocol, Union, TypedDict,
    cast
)

# ── Native Binding Import ────────────────────────────────────────────────
try:
    import _auwgent_sdk as _native
except ImportError:
    # Fallback for static analysis in some environments
    _native = Any 

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
    stack: List[str]
    initialInput: Optional[Any]

class PartialIntentEnvelope(TypedDict, total=False):
    partial: bool
    complete: bool
    mode: str
    segment: int
    raw: str

class PartialTextIntentValue(PartialIntentEnvelope, total=False):
    text: str
    delta: str

class PartialStructuredIntentValue(PartialIntentEnvelope, total=False):
    type: str
    args: Any
    response: Any
    result: Any
    error: Any
    message: str

PartialIntentValue = Union[PartialTextIntentValue, PartialStructuredIntentValue, Dict[str, Any]]
IntentValue = Dict[str, Any]

class IRComponentChildrenDef(TypedDict, total=False):
    kind: str
    components: List[str]

class IRComponentActionTargetDef(TypedDict, total=False):
    name: str
    params: Any

class IRComponentDef(TypedDict, total=False):
    name: str
    props: Any
    action: Dict[str, List[IRComponentActionTargetDef]]
    children: IRComponentChildrenDef

class AgentIRShape(TypedDict, total=False):
    name: str
    input: Any
    output: Any
    context: Any
    tools: List[Dict[str, Any]]
    workflows: List[Dict[str, Any]]
    helpers: List[Dict[str, Any]]
    components: List[IRComponentDef]
    types: Dict[str, Any]
    modelConfig: List[Dict[str, Any]]
    customIntents: List[Dict[str, Any]]

# ── Error Types ───────────────────────────────────────────────────────────

class AuwgentToolError(Exception):
    """Raised when a tool execution fails inside the engine."""

    def __init__(self, tool: str, message: str):
        self.tool = tool
        self.message = message
        super().__init__(f"Tool '{tool}' failed: {message}")

# ── Middleware Types ──────────────────────────────────────────────────────

class MiddlewareContext(TypedDict):
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

    async def onRunStart(self, session: SessionState, ctx: MiddlewareContext) -> SessionState: ...
    async def onLLMStart(self, prompt: str, ctx: MiddlewareContext) -> Optional[str]: ...
    async def onIntent(self, name: str, value: IntentValue, ctx: MiddlewareContext) -> Optional[SessionState]: ...
    async def onLLMEnd(self, response: Dict[str, Any], ctx: MiddlewareContext) -> None: ...
    async def onRunComplete(self, finalSession: SessionState, ctx: MiddlewareContext) -> None: ...
    async def onError(self, error: Exception, session: Optional[SessionState], ctx: MiddlewareContext) -> bool: ...

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
        self._native = _native.AuwgentNative(ir_json)
        self.ir: Dict[str, Any] = ir if isinstance(ir, dict) else json.loads(ir_json)
        self.middleware: List[Any] = [*cast(list, config.get("middleware", []))]
        self._shared_context: Dict[str, Any] = {}
        self._agent_stack: List[str] = []

        # Deferred handler storage — registered/deregistered around each run()
        self._stored_intent_handler: Optional[Callable] = None
        self._stored_partial_handler: Optional[Callable] = None
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

        Runtime-emitted intent names can include tool/workflow/helper events,
        `component`, `render_component`, `response_text`, `response_schema`, and `error`.
        """
        self._stored_intent_handler = callback

    def on_intent_partial(self, callback: Callable[[str, PartialIntentValue], None]) -> None:
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

    def on_handlers_partial(self, handlers: Dict[str, Callable[[PartialIntentValue], None]]) -> None:
        """
        Register multiple partial intent handlers using an object-style API.
        """
        def dispatch(name: str, value: PartialIntentValue) -> None:
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
            systemPrompt=None,
            embed=self.embed,
            embedBatch=self.embed_batch,
            set_context=self.set_context
        )
        for k, v in self._shared_context.items():
            ctx[k] = v  # type: ignore
        return ctx

    def _build_context_from_runtime_event(self, event: Dict[str, Any]) -> MiddlewareContext:
        ctx = self._build_context()
        runtime_ctx = event.get("context")

        if isinstance(runtime_ctx, dict):
            active_agent = runtime_ctx.get("activeAgent")
            if isinstance(active_agent, str):
                ctx["activeAgent"] = active_agent

            stack = runtime_ctx.get("stack")
            if isinstance(stack, list):
                ctx["stack"] = list(stack)
                self._agent_stack = list(stack)

            root_agent = runtime_ctx.get("rootAgent")
            if isinstance(root_agent, str):
                ctx["rootAgent"] = root_agent

            raw_block = runtime_ctx.get("rawBlock")
            if isinstance(raw_block, str):
                ctx["rawBlock"] = raw_block
                self._last_raw_block = raw_block

            system_prompt = runtime_ctx.get("systemPrompt")
            if isinstance(system_prompt, str):
                ctx["systemPrompt"] = system_prompt

        return ctx

    async def _handle_middleware_event(self, event_json: str) -> Optional[str]:
        event = cast(Dict[str, Any], json.loads(event_json))
        ctx = self._build_context_from_runtime_event(event)
        event_type = event.get("type")

        if event_type == "intent":
            value = event.get("value")
            if isinstance(value, dict):
                value = dict(value)
                value.pop("_raw", None)

            for middleware in self._get_middleware(ctx):
                if hasattr(middleware, "onIntent"):
                    control = await middleware.onIntent(cast(str, event.get("name", "")), value, ctx)
                    if control is not None:
                        return json.dumps(control)
            return None

        if event_type == "llm_start":
            current_prompt = event.get("prompt", "")
            if not isinstance(current_prompt, str):
                current_prompt = ""

            for middleware in self._get_middleware(ctx):
                if hasattr(middleware, "onLLMStart"):
                    result = await middleware.onLLMStart(current_prompt, ctx)
                    if isinstance(result, str):
                        current_prompt = result

            return json.dumps({
                "prompt": current_prompt,
                "stack": ctx.get("stack")
            })

        if event_type == "llm_end":
            for middleware in self._get_middleware(ctx):
                if hasattr(middleware, "onLLMEnd"):
                    await middleware.onLLMEnd(event.get("response") or {}, ctx)
            return None

        if event_type == "run_start":
            session = cast(SessionState, event.get("session", {}))
            for middleware in self._get_middleware(ctx):
                if hasattr(middleware, "onRunStart"):
                    session = await middleware.onRunStart(session, ctx)

            if "stack" in ctx and isinstance(ctx["stack"], list):
                session["stack"] = list(ctx["stack"])
                self._agent_stack = list(ctx["stack"])

            return json.dumps({"session": session})

        if event_type == "run_complete":
            session = cast(SessionState, event.get("session", {}))
            for middleware in self._get_middleware(ctx):
                if hasattr(middleware, "onRunComplete"):
                    await middleware.onRunComplete(session, ctx)
            return None

        if event_type == "error":
            error_payload = event.get("error", {})
            if isinstance(error_payload, dict) and error_payload.get("kind") == "tool_error":
                error: Exception = AuwgentToolError(
                    cast(str, error_payload.get("tool", "unknown")),
                    cast(str, error_payload.get("message", "unknown")),
                )
            else:
                message = error_payload.get("message", "Unknown runtime error") if isinstance(error_payload, dict) else "Unknown runtime error"
                error = RuntimeError(message)

            session = cast(Optional[SessionState], event.get("session"))
            for middleware in self._get_middleware(ctx):
                if hasattr(middleware, "onError"):
                    swallow = await middleware.onError(error, session, ctx)
                    if swallow:
                        return json.dumps({"swallow": True})
            return None

        return None

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
        user_handler = self._stored_intent_handler

        async def wrap_intent(name: str, value_json_str: str, agent_name: str) -> Optional[str]:
            val_dict: Dict[str, Any] = json.loads(value_json_str)
            ctx = self._build_context()
            ctx["activeAgent"] = agent_name

            # Extract _raw from Rust-injected field
            raw_block = val_dict.get("_raw")
            if raw_block is not None:
                ctx["rawBlock"] = raw_block
                self._last_raw_block = raw_block
                val_dict.pop("_raw", None)

            if user_handler:
                sig = inspect.signature(user_handler)
                if len(sig.parameters) >= 3:
                    res = await user_handler(name, val_dict, agent_name)
                else:
                    res = await user_handler(name, val_dict)
                return json.dumps(res) if res is not None else None
            return None

        self._native.on_intent(wrap_intent)

        partial_handler = self._stored_partial_handler
        def wrap_partial(name: str, value_json_str: str, agent_name: str) -> None:
            if partial_handler is not None:
                value_dict = json.loads(value_json_str)
                sig = inspect.signature(partial_handler)
                if len(sig.parameters) >= 3:
                    partial_handler(name, value_dict, agent_name)
                else:
                    partial_handler(name, value_dict)
        self._native.on_intent_partial(wrap_partial)
        self._native.on_middleware_event(self._handle_middleware_event)

        async def wrap_sub_start(helper_name: str, session_json: str) -> Optional[str]:
            session = cast(SessionState, json.loads(session_json))
            if "stack" in session:
                self._agent_stack = list(session["stack"])
            return json.dumps(session)
        self._native.on_sub_engine_start(wrap_sub_start)

        async def wrap_sub_complete(helper_name: str, session_json: str) -> None:
            session = cast(SessionState, json.loads(session_json))
            if "stack" in session:
                self._agent_stack = list(session["stack"])
        self._native.on_sub_engine_complete(wrap_sub_complete)

    def _deactivate_listeners(self) -> None:
        self._native.clear_listeners()

    # ── Execution ─────────────────────────────────────────────────────────

    async def run(self, input_val: Any = None) -> SessionState:
        """Run the agentic loop. Returns the exported session state."""
        self._shared_context = {}
        self._agent_stack = [self.ir.get("name", "agent")]
        self._last_raw_block = None

        self._activate_listeners()

        try:
            # Pass raw strings directly; only JSON-encode non-string types
            if input_val is None:
                input_str = None
            elif isinstance(input_val, str):
                input_str = input_val
            else:
                input_str = json.dumps(input_val)

            res_json = await self._native.run(
                input_str, 
                None
            )
            current_session = json.loads(res_json)

            # Sync stack back after run
            if "stack" in current_session:
                self._agent_stack = list(current_session["stack"])

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

    def generate_prompt(self, helper_name: Optional[str] = None) -> str:
        """Generate the system prompt (debugging)."""
        return self._native.generate_prompt(helper_name)

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
