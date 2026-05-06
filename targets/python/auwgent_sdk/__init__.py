"""
Auwgent Runtime — Type-Safe Python Wrapper

This module provides a type-safe layer on top of the native Rust binding (PyO3).
It mirrors the TypeScript SDK's feature set: deferred listeners, middleware pipeline,
object-style intent handlers, and proper session lifecycle management.
"""

import json
import inspect
import sys
import asyncio
import typing
from datetime import datetime
from typing import (
    Any, Callable, Awaitable, Dict, List, Optional,
    TypeVar, Generic, Protocol, Union, TypedDict,
    Type, ClassVar, cast, runtime_checkable
)


# ── Native Binding Import ────────────────────────────────────────────────
try:
    from . import _auwgent_sdk as _native
except ImportError:
    try:
        import _auwgent_sdk as _native
    except ImportError:
        # Fallback for static analysis in some environments
        _native = Any  # type: ignore

# ── Type Variables ────────────────────────────────────────────────────────
AgentIR = TypeVar("AgentIR")
AgentContext = TypeVar("AgentContext")
AgentOutput = TypeVar("AgentOutput")
AgentTools = TypeVar("AgentTools")

# ── Session State ─────────────────────────────────────────────────────────

class SessionTurn(TypedDict):
    input: str
    model_response: str

class _RequiredSessionState(TypedDict):
    turns: List[SessionTurn]
    stack: List[str]

class BindingCursor(TypedDict):
    turnIndex: Optional[int]
    role: str
    input: Optional[str]

class SessionState(_RequiredSessionState, total=False):
    """Exported session state from the engine."""
    systemPrompt: Optional[str]
    initialInput: Optional[Any]
    bindingCursor: Optional[BindingCursor]

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

class TokenUsage(TypedDict):
    prompt_tokens: int
    completion_tokens: int
    total_tokens: int
    reasoning_tokens: int
    cached_tokens: int

class TurnMetadata(TypedDict):
    turn_index: int
    usage: TokenUsage
    finish_reason: Optional[Union[str, Dict[str, str]]]
    model: str

class AggregateUsage(TypedDict):
    prompt_tokens: int
    completion_tokens: int
    total_tokens: int
    reasoning_tokens: int
    cached_tokens: int

class RunMetadata(TypedDict):
    aggregate: AggregateUsage
    turns: List[TurnMetadata]

class AuwgentWarning(TypedDict, total=False):
    timestamp: str
    source: str
    message: str
    detail: Optional[str]
    agentName: Optional[str]

WarningCallback = Callable[[AuwgentWarning], None]

# ── Error Types ───────────────────────────────────────────────────────────

class AuwgentToolError(Exception):
    """Raised when a tool execution fails inside the engine."""

    def __init__(self, tool: str, message: str):
        self.tool = tool
        self.message = message
        super().__init__(f"Tool '{tool}' failed: {message}")

# ── Middleware Types ──────────────────────────────────────────────────────

class MiddlewareContext(Dict[str, Any]):
    activeAgent: str
    stack: List[str]
    rootAgent: str
    rawBlock: Optional[str]
    systemPrompt: Optional[str]
    embed: Callable[[str], Awaitable[List[float]]]
    embedBatch: Callable[[List[str]], Awaitable[List[List[float]]]]
    set_context: Callable[[Any], None]

@runtime_checkable
class Middleware(Protocol):
    name: ClassVar[str]
    target: ClassVar[Optional[Union[str, List[str]]]]

    async def onRunStart(self, session: SessionState, ctx: MiddlewareContext) -> SessionState: ...
    async def onLLMStart(self, prompt: str, ctx: MiddlewareContext) -> Optional[str]: ...
    async def onIntent(self, name: str, value: IntentValue, ctx: MiddlewareContext) -> Optional[SessionState]: ...
    async def onIntentPartial(self, name: str, value: PartialIntentValue, ctx: MiddlewareContext) -> None: ...
    async def onLLMEnd(self, response: Dict[str, Any], ctx: MiddlewareContext) -> None: ...
    async def onRunComplete(self, finalSession: SessionState, ctx: MiddlewareContext) -> None: ...
    async def onError(self, error: Exception, session: Optional[SessionState], ctx: MiddlewareContext) -> bool: ...

# Accept both instances and class types in middleware lists
MiddlewareEntry = Union[Middleware, Type[Any]]

_RESERVED_MIDDLEWARE_CONTEXT_KEYS: set[str] = {
    "activeAgent",
    "stack",
    "rootAgent",
    "rawBlock",
    "systemPrompt",
    "embed",
    "embedBatch",
    "set_context",
}


# ── Configuration ────────────────────────────────────────────────────────

class AuwgentConfig(TypedDict, total=False):
    tools: Union[Dict[str, Callable[..., Awaitable[Any]]], Any]
    middleware: List[MiddlewareEntry]
    context: Dict[str, Any]
    apiKeys: Dict[str, str]


# ── Type-Safe Auwgent Wrapper ─────────────────────────────────────────────

class TypedAuwgent(Generic[AgentIR, AgentContext, AgentOutput, AgentTools]):
    """
    Production-grade, type-safe wrapper around the Native Rust Engine (PyO3).

    Mirrors the TypeScript SDK's full feature set:
    - Deferred listener registration (activate/deactivate around each run)
    - Middleware pipeline with onIntent, onLLMStart/End, onRunStart/Complete
    - Class-based intent handlers via on_intent() / on_intent_partial()
    - Session import/export for persistence
    """

    def __init__(self, ir: Any, config: AuwgentConfig):
        ir_json = json.dumps(ir) if isinstance(ir, dict) else ir
        self._native = _native.AuwgentNative(ir_json)
        self.ir: Dict[str, Any] = ir if isinstance(ir, dict) else json.loads(ir_json)
        # Auto-instantiate class-type middleware entries
        raw_middleware = cast(list, config.get("middleware", []))
        self.middleware: List[Any] = [
            m() if isinstance(m, type) else m
            for m in raw_middleware
        ]
        self._shared_context: Dict[str, Any] = {}
        self._agent_stack: List[str] = []
        self._helper_sessions: Dict[str, SessionState] = {}
        self._warnings: List[AuwgentWarning] = []
        self._warning_handler: Optional[WarningCallback] = None
        self._background_tasks: set[asyncio.Task[Any]] = set()


        # Deferred handler storage — registered/deregistered around each run()
        self._stored_intent_handler: Optional[Any] = None
        self._stored_partial_handler: Optional[Any] = None
        self._last_raw_block: Optional[str] = None
        self._running_loop: Optional[asyncio.AbstractEventLoop] = None

        # ── 1. Context ──
        if "context" in config:
            self.set_context(config["context"])

        # ── 2. Drivers ──
        api_keys = cast(Dict[str, str], config.get("apiKeys", {}))
        if "geminiApiKey" in api_keys:
            self.set_gemini_driver(api_keys["geminiApiKey"])
        if "openaiApiKey" in api_keys:
            self.set_openai_driver(api_keys["openaiApiKey"])
        if "groqApiKey" in api_keys:
            self.set_groq_driver(api_keys["groqApiKey"])

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

    def set_groq_driver(self, api_key: str) -> None:
        self._native.set_groq_driver(api_key)

    def set_custom_driver(self, id: str, api_key: str, base_url: str) -> None:
        self._native.set_custom_driver(id, api_key, base_url)

    def set_context(self, context: Any) -> None:
        self._native.set_context(json.dumps(context))

    async def embed(self, text: str) -> List[float]:
        """Generate an embedding for the given text using the configured model."""
        return await self._native.embed(text)

    async def embed_batch(self, texts: List[str]) -> List[List[float]]:
        """Generate embeddings for a batch of texts."""
        return await self._native.embed_batch(texts)

    # ── Tool Registration ─────────────────────────────────────────────────

    def register_tool(self, name: str, callback: Callable[..., Any]) -> None:
        def wrap_callback(args_json_str: str) -> str:
            args_dict = json.loads(args_json_str)
            if not isinstance(args_dict, dict):
                args_dict = {}

            res = callback(**args_dict)
            if inspect.isawaitable(res):
                loop = self._running_loop
                if loop is None or loop.is_closed():
                    raise RuntimeError(
                        "async Python tool was called without an active Auwgent run loop"
                    )
                res = asyncio.run_coroutine_threadsafe(
                    cast(Awaitable[Any], res),
                    loop,
                ).result()

            return json.dumps(res)
        self._native.register_tool(name, wrap_callback)

    # ── Intent Handlers ───────────────────────────────────────────────────

    def on_intent(self, handler: Any) -> None:
        """
        Register a class/object intent handler.

        Handler methods are resolved by intent name (exact then sanitized), e.g.
        response_text, tool_call, workflow_result, component, render_component.
        Method signature must be: (value, agent_name)
        """
        if isinstance(handler, type):
            handler = handler()
        elif callable(handler):
            raise TypeError("on_intent expects a class/object handler, not a function callback")
        self._stored_intent_handler = handler

    def on_intent_partial(self, handler: Any) -> None:
        """
        Register a class/object partial-intent handler.

        Method signature must be: (value, agent_name)
        """
        if isinstance(handler, type):
            handler = handler()
        elif callable(handler):
            raise TypeError("on_intent_partial expects a class/object handler, not a function callback")
        self._stored_partial_handler = handler

    def _intent_method_name(self, name: str) -> str:
        sanitized = "".join(c if (c.isalnum() or c == "_") else "_" for c in name)
        if sanitized and sanitized[0].isdigit():
            sanitized = f"_{sanitized}"
        return (sanitized or "intent").lower()

    async def _dispatch_intent_handler(
        self,
        handler: Any,
        name: str,
        value: Dict[str, Any],
        agent_name: str,
    ) -> Optional[Dict[str, Any]]:
        method = getattr(handler, name, None)
        if method is None:
            method = getattr(handler, self._intent_method_name(name), None)
        if method is None or not callable(method):
            return None

        result = method(value, agent_name)

        if inspect.isawaitable(result):
            result = await result
        return cast(Optional[Dict[str, Any]], result)

    def _dispatch_partial_handler(
        self,
        handler: Any,
        name: str,
        value: PartialIntentValue,
        agent_name: str,
    ) -> None:
        method = getattr(handler, name, None)
        if method is None:
            method = getattr(handler, self._intent_method_name(name), None)
        if method is None or not callable(method):
            return

        result = method(value, agent_name)
        if inspect.isawaitable(result):
            self._schedule_background_awaitable(
                cast(Awaitable[Any], result),
                source="onIntentPartial",
                message="partial intent async handler failed",
                agent_name=agent_name,
            )

    def _schedule_background_awaitable(
        self,
        awaitable: Awaitable[Any],
        source: str,
        message: str,
        agent_name: str,
    ) -> None:
        try:
            loop = asyncio.get_running_loop()
        except RuntimeError:
            self._report_warning(source, "no running event loop for async handler", agent_name=agent_name)
            return

        task = loop.create_task(awaitable)
        self._background_tasks.add(task)

        def _on_done(done_task: asyncio.Task[Any]) -> None:
            self._background_tasks.discard(done_task)
            try:
                done_task.result()
            except Exception as error:
                self._report_warning(source, message, error, agent_name)

        task.add_done_callback(_on_done)

    def on_warning(self, callback: WarningCallback) -> None:
        """Register a callback for non-fatal SDK/runtime integration warnings."""
        self._warning_handler = callback

    def get_warnings(self) -> List[AuwgentWarning]:
        """Return collected non-fatal warnings."""
        return [dict(w) for w in self._warnings]

    def clear_warnings(self) -> None:
        """Clear collected non-fatal warnings."""
        self._warnings.clear()

    def _report_warning(self, source: str, message: str, error: Optional[Exception] = None, agent_name: Optional[str] = None) -> None:
        detail: Optional[str] = None
        if error is not None:
            detail = str(error)

        warning: AuwgentWarning = {
            "timestamp": datetime.utcnow().isoformat() + "Z",
            "source": source,
            "message": message,
            "detail": detail,
            "agentName": agent_name,
        }
        self._warnings.append(warning)

        if self._warning_handler is not None:
            try:
                self._warning_handler(warning)
            except Exception:
                pass

        detail_suffix = f": {detail}" if detail else ""
        agent_suffix = f" [agent={agent_name}]" if agent_name else ""
        print(f"[auwgent][{source}]{agent_suffix} {message}{detail_suffix}", file=sys.stderr)

    # ── Middleware Helpers ─────────────────────────────────────────────────

    def _build_context(self) -> MiddlewareContext:
        active = self._agent_stack[-1] if self._agent_stack else self.ir.get("name", "agent")
        def set_context_and_mirror(data: Any) -> None:
            self.set_context(data)
            if isinstance(data, dict):
                for key, value in data.items():
                    self._shared_context[key] = value
                    ctx[key] = value  # type: ignore
            else:
                self._shared_context["dynamic_context"] = data
                ctx["dynamic_context"] = data  # type: ignore

        ctx = MiddlewareContext(
            activeAgent=active,
            stack=list(self._agent_stack),
            rootAgent=self.ir.get("name", "agent"),
            rawBlock=self._last_raw_block,
            systemPrompt=None,
            embed=self.embed,
            embedBatch=self.embed_batch,
            set_context=set_context_and_mirror
        )
        for k, v in self._shared_context.items():
            ctx[k] = v  # type: ignore
        return ctx

    def _persist_middleware_context(self, ctx: MiddlewareContext) -> None:
        for key, value in ctx.items():
            if key not in _RESERVED_MIDDLEWARE_CONTEXT_KEYS:
                self._shared_context[key] = value

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
        try:
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
                        try:
                            control = await middleware.onIntent(cast(str, event.get("name", "")), value, ctx)
                            self._persist_middleware_context(ctx)
                            if control is not None:
                                return json.dumps(control)
                        except Exception as error:
                            self._report_warning("middleware", "middleware onIntent threw", error, cast(str, ctx.get("activeAgent", "")))
                self._persist_middleware_context(ctx)
                return None

            if event_type == "llm_start":
                current_prompt = event.get("prompt", "")
                if not isinstance(current_prompt, str):
                    current_prompt = ""

                for middleware in self._get_middleware(ctx):
                    if hasattr(middleware, "onLLMStart"):
                        try:
                            result = await middleware.onLLMStart(current_prompt, ctx)
                            self._persist_middleware_context(ctx)
                            if isinstance(result, str):
                                current_prompt = result
                        except Exception as error:
                            self._report_warning("middleware", "middleware onLLMStart threw", error, cast(str, ctx.get("activeAgent", "")))

                self._persist_middleware_context(ctx)

                return json.dumps({
                    "prompt": current_prompt,
                    "stack": ctx.get("stack")
                })

            if event_type == "llm_end":
                for middleware in self._get_middleware(ctx):
                    if hasattr(middleware, "onLLMEnd"):
                        try:
                            await middleware.onLLMEnd(event.get("response") or {}, ctx)
                            self._persist_middleware_context(ctx)
                        except Exception as error:
                            self._report_warning("middleware", "middleware onLLMEnd threw", error, cast(str, ctx.get("activeAgent", "")))
                self._persist_middleware_context(ctx)
                return None

            if event_type == "run_start":
                session = event.get("session")
                if session is None:
                    session = {}
                session = cast(SessionState, session)

                for middleware in self._get_middleware(ctx):
                    if hasattr(middleware, "onRunStart"):
                        try:
                            result = await middleware.onRunStart(session, ctx)
                            if result is not None:
                                session = result
                            self._persist_middleware_context(ctx)
                        except Exception as error:
                            self._report_warning("middleware", "middleware onRunStart threw", error, cast(str, ctx.get("activeAgent", "")))

                self._persist_middleware_context(ctx)

                if "stack" in ctx and isinstance(ctx["stack"], list):
                    session["stack"] = list(ctx["stack"])
                    self._agent_stack = list(ctx["stack"])

                return json.dumps({"session": session})

            if event_type == "run_complete":
                session = cast(SessionState, event.get("session", {}))
                for middleware in self._get_middleware(ctx):
                    if hasattr(middleware, "onRunComplete"):
                        try:
                            await middleware.onRunComplete(session, ctx)
                            self._persist_middleware_context(ctx)
                        except Exception as error:
                            self._report_warning("middleware", "middleware onRunComplete threw", error, cast(str, ctx.get("activeAgent", "")))
                self._persist_middleware_context(ctx)
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
                        try:
                            swallow = await middleware.onError(error, session, ctx)
                            self._persist_middleware_context(ctx)
                            if swallow:
                                return json.dumps({"swallow": True})
                        except Exception as middleware_error:
                            self._report_warning("middleware", "middleware onError threw", middleware_error, cast(str, ctx.get("activeAgent", "")))
                self._persist_middleware_context(ctx)
                return None

            return None
        except Exception as error:
            import traceback
            traceback.print_exc()
            self._report_warning("onMiddlewareEvent", "failed to handle middleware event", error)
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
            try:
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
                    res = await self._dispatch_intent_handler(user_handler, name, val_dict, agent_name)
                    return json.dumps(res) if res is not None else None
                return None
            except Exception as error:
                self._report_warning("onIntent", "intent callback failed", error, agent_name)
                return None

        self._native.on_intent(wrap_intent)

        partial_handler = self._stored_partial_handler
        def wrap_partial(name: str, value_json_str: str, agent_name: str) -> None:
            if partial_handler is not None:
                try:
                    value_dict = json.loads(value_json_str)
                    self._dispatch_partial_handler(partial_handler, name, value_dict, agent_name)
                except Exception as error:
                    self._report_warning("onIntentPartial", "partial intent callback failed", error, agent_name)
        self._native.on_intent_partial(wrap_partial)
        async def wrap_middleware_event(event_json: str) -> Optional[str]:
            try:
                return await self._handle_middleware_event(event_json)
            except Exception as error:
                self._report_warning("onMiddlewareEvent", "middleware event callback failed", error)
                return None
        self._native.on_middleware_event(wrap_middleware_event)

        async def wrap_sub_start(helper_name: str, session_json: str) -> Optional[str]:
            try:
                session = self._helper_sessions.get(helper_name) or cast(SessionState, json.loads(session_json))
                if "stack" in session:
                    self._agent_stack = list(session["stack"])
                return json.dumps(session)
            except Exception as error:
                self._report_warning("onSubEngineStart", "sub-engine start callback failed", error, helper_name)
                return session_json
        self._native.on_sub_engine_start(wrap_sub_start)

        async def wrap_sub_complete(helper_name: str, session_json: str) -> None:
            try:
                session = cast(SessionState, json.loads(session_json))
                self._helper_sessions[helper_name] = session
                if "stack" in session:
                    self._agent_stack = list(session["stack"])
            except Exception as error:
                self._report_warning("onSubEngineComplete", "sub-engine complete callback failed", error, helper_name)
        self._native.on_sub_engine_complete(wrap_sub_complete)

    def _deactivate_listeners(self) -> None:
        self._native.clear_listeners()

    # ── Execution ─────────────────────────────────────────────────────────

    async def run(self, input_val: Any = None) -> SessionState:
        """Run the agentic loop. Returns the exported session state."""
        self._shared_context = {}
        self._agent_stack = [self.ir.get("name", "agent")]
        self._last_raw_block = None
        self._running_loop = asyncio.get_running_loop()

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
        except Exception as error:
            self._report_warning("run", "native run failed", error)
            raise
        finally:
            self._running_loop = None
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
        self._helper_sessions.clear()

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

    def get_metadata(self) -> RunMetadata:
        """Get the exact token usage and prompt/response telemetry from the last full run()."""
        return cast(RunMetadata, json.loads(self._native.get_metadata()))

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
