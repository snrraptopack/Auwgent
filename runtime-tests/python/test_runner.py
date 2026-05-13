#!/usr/bin/env python3
"""
Auwgent Runtime Test Runner - Python

Exercises the full FFI + real LLM stack for every canonical scenario.
Run with: python test_runner.py
"""

import asyncio
import json
import os
import sys
from typing import Any, Dict, List, Optional

# Ensure auwgent_sdk is findable
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "targets", "python")))

from canonical_types import (
    AuwgentConfig, auwgent,
    AuwgentBaseIntentHandler, AuwgentBasePartialIntentHandler,
    AuwgentMiddleware,AuwgentTools
)
from auwgent_sdk import SessionState

GROQ_API_KEY = os.environ.get("GROQ_API_KEY", "")
if not GROQ_API_KEY:
    print("FAIL GROQ_API_KEY is required. Set it in your environment or .env file.")
    sys.exit(1)


# ===============================================================================
# TEST INFRASTRUCTURE
# ===============================================================================
#

class Tools(AuwgentTools):
    async def get_location(self):
        return "Accra Ghana"

    async def get_marks(self,id):
        return f"marks for {id} : A, B, C , D"

def create_base_config() -> AuwgentConfig:
    return {
        "apiKeys": {"groqApiKey": GROQ_API_KEY},
        "context": {"user_name": "TestUser", "age": 25, "id": "test-123"},
        "tools": Tools()
    }


def print_header(title: str):
    print(f"\n{'=' * 70}")
    print(f" SCENARIO: {title}")
    print(f"{'=' * 70}")


def print_report(
    name: str,
    passed: bool,
    events: List[Dict[str, Any]],
    partials: List[Dict[str, Any]],
    session: Optional[SessionState],
    metadata: Any,
    middleware_log: List[str],
    error: Optional[str] = None,
    notes: Optional[List[str]] = None,
):
    notes = notes or []
    print(f"\n  Status: {'PASS PASSED' if passed else 'FAIL FAILED'}")
    if error:
        print(f"  Error: {error}")

    print(f"\n  -- Intent Events ({len(events)}) --")
    for ev in events:
        payload = json.dumps(ev["value"])[:200]
        print(f"    [{ev['name']}] agent={ev['agentName']} value={payload}")

    print(f"\n  -- Partial Events ({len(partials)}) --")
    if partials:
        unique = sorted({p["name"] for p in partials})
        print(f"    Unique partial intents: {', '.join(unique)}")
    else:
        print("    (none)")

    print("\n  -- Session --")
    if session:
        print(f"    Turns: {len(session.get('turns', []))}")
        print(f"    Stack: {json.dumps(session.get('stack', []))}")
        for i, turn in enumerate(session.get("turns", [])):
            resp = (turn.get("model_response", "") or "")[:120].replace("\n", "\\n")
            inp = (turn.get("input", "") or "")[:80]
            print(f"    Turn {i}: input=\"{inp}\" response=\"{resp}...\"")

    print("\n  -- Metadata --")
    if metadata:
        agg = metadata.get("aggregate", {})
        print(f"    Total tokens: {agg.get('total_tokens', '?')}")
        print(f"    Turns: {len(metadata.get('turns', []))}")

    if middleware_log:
        print(f"\n  -- Middleware Log ({len(middleware_log)}) --")
        for entry in middleware_log:
            print(f"    -> {entry}")

    if notes:
        print("\n  -- Notes --")
        for note in notes:
            print(f"    {note}")


async def run_scenario(
    name: str,
    setup,
    input_text: str,
) -> Dict[str, Any]:
    print_header(name)
    print(f"\n  Input: \"{input_text}\"")

    events: List[Dict[str, Any]] = []
    partials: List[Dict[str, Any]] = []
    result: Dict[str, Any] = {
        "name": name,
        "passed": False,
        "events": events,
        "partials": partials,
        "session": None,
        "metadata": None,
        "middlewareLog": [],
        "error": None,
        "notes": [],
    }

    try:
        agent, middleware_log = setup()
        result["middlewareLog"] = middleware_log

        class IntentCollector(AuwgentBaseIntentHandler):
            def _collect(self, name: str, value: Any, agent_name: str):
                events.append({"name": name, "value": value, "agentName": agent_name})
                return None

            def response_text(self, value, agent_name):
                return self._collect("response_text", value, agent_name)
            def response_schema(self, value, agent_name):
                return self._collect("response_schema", value, agent_name)
            def tool_call(self, value, agent_name):
                return self._collect("tool_call", value, agent_name)
            def tool_result(self, value, agent_name):
                return self._collect("tool_result", value, agent_name)
            def tool_error(self, value, agent_name):
                return self._collect("tool_error", value, agent_name)
            def tool_skipped(self, value, agent_name):
                return self._collect("tool_skipped", value, agent_name)
            def workflow_call(self, value, agent_name):
                return self._collect("workflow_call", value, agent_name)
            def workflow_result(self, value, agent_name):
                return self._collect("workflow_result", value, agent_name)
            def helper_call(self, value, agent_name):
                return self._collect("helper_call", value, agent_name)
            def helper_result(self, value, agent_name):
                return self._collect("helper_result", value, agent_name)
            def loud(self, value, agent_name):
                return self._collect("Loud", value, agent_name)
            def error(self, value, agent_name):
                return self._collect("error", value, agent_name)

        class PartialCollector(AuwgentBasePartialIntentHandler):
            def _collect(self, name: str, value: Any, agent_name: str):
                partials.append({"name": name, "value": value, "agentName": agent_name})

            def response_text(self, value, agent_name):
                self._collect("response_text", value, agent_name)
            def tool_call(self, value, agent_name):
                self._collect("tool_call", value, agent_name)
            def helper_call(self, value, agent_name):
                self._collect("helper_call", value, agent_name)
            def helper_result(self, value, agent_name):
                self._collect("helper_result", value, agent_name)
            def loud(self, value, agent_name):
                self._collect("Loud", value, agent_name)

        agent.on_intent(IntentCollector())
        agent.on_intent_partial(PartialCollector())

        session = await agent.run(input_text)
        result["session"] = session
        result["metadata"] = agent.get_metadata()
        result["passed"] = True

        if not session.get("turns"):
            result["notes"].append("! No turns recorded in session")
    except Exception as e:
        result["error"] = str(e)
        result["notes"].append(f"!!! Exception: {e}")

    print_report(
        result["name"],
        result["passed"],
        result["events"],
        result["partials"],
        result["session"],
        result["metadata"],
        result["middlewareLog"],
        result["error"],
        result["notes"],
    )
    return result


# ===============================================================================
# SCENARIOS
# ===============================================================================

async def scenario1_basic_chat():
    def setup():
        agent = auwgent(create_base_config())
        return agent, []
    return await run_scenario("1. Basic Chat", setup, "Hello! Please just say hi back in a friendly way.")


async def scenario2_tool_no_args():
    def setup():
        agent = auwgent(create_base_config())
        return agent, []
    return await run_scenario("2. Tool Call (no args)", setup, "What is my current location?")


async def scenario3_tool_with_args():
    def setup():
        agent = auwgent(create_base_config())
        return agent, []
    return await run_scenario("3. Tool Call (with args)", setup, "Get my marks. My user id is test-123.")


async def scenario4_workflow():
    def setup():
        agent = auwgent(create_base_config())
        return agent, []
    return await run_scenario("4. Workflow Execution", setup, "Get my location and marks together. My user id is test-123.")


async def scenario5_helper_return():
    def setup():
        agent = auwgent(create_base_config())
        return agent, []
    return await run_scenario("5. Helper (Return handoff)", setup, "I need a plan for how to learn Rust programming.")


async def scenario6_helper_user():
    def setup():
        agent = auwgent(create_base_config())
        return agent, []
    return await run_scenario("6. Helper (User handoff)", setup, "Tell me a joke about programming.")


async def scenario7_custom_intent():
    def setup():
        agent = auwgent(create_base_config())
        return agent, []
    return await run_scenario("7. Custom Intent (Loud)", setup, "Explain out loud what you are going to do next.")


async def scenario8_middleware_lifecycle():
    log: List[str] = []

    class LifecycleLogger(AuwgentMiddleware):
        name = "LifecycleLogger"

        async def onRunStart(self, session, ctx):
            log.append(f"run_start | activeAgent={ctx.get('activeAgent')} | stack={json.dumps(ctx.get('stack'))}")
            return session

        async def onLLMStart(self, prompt, ctx):
            log.append(f"llm_start | promptLen={len(prompt)} | activeAgent={ctx.get('activeAgent')}")
            return prompt

        async def onIntent(self, name, value, ctx):
            log.append(f"intent | {name} | activeAgent={ctx.get('activeAgent')}")
            return None

        async def onLLMEnd(self, response, ctx):
            log.append(f"llm_end | activeAgent={ctx.get('activeAgent')}")

        async def onRunComplete(self, finalSession, ctx):
            log.append(f"run_complete | turns={len(finalSession.get('turns', []))} | activeAgent={ctx.get('activeAgent')}")

        async def onError(self, error, session, ctx):
            log.append(f"error | {error} | activeAgent={ctx.get('activeAgent')}")
            return False

    def setup():
        config = create_base_config()
        config["middleware"] = [LifecycleLogger()]
        agent = auwgent(config)
        return agent, log

    return await run_scenario("8. Middleware Lifecycle", setup, "Say hello and then ask for my location.")


async def scenario9_session_export_import():
    config = create_base_config()
    agent = auwgent(config)

    print(f"\n{'-' * 70}")
    print("  Phase 1: Initial run")
    print(f"{'-' * 70}")
    await agent.run("My name is RuntimeTestUser.")
    session1 = agent.export_session()
    print(f"  Exported session with {len(session1.get('turns', []))} turn(s)")

    print(f"\n{'-' * 70}")
    print("  Phase 2: Fresh agent with imported session")
    print(f"{'-' * 70}")
    agent2 = auwgent(config)
    agent2.import_session(session1)

    def setup():
        return agent2, []

    return await run_scenario("9. Session Export/Import", setup, "What is my name? (You should remember it from earlier.)")


async def scenario10_error_swallowing():
    log: List[str] = []

    class ErrorSwallower(AuwgentMiddleware):
        name = "ErrorSwallower"

        async def onError(self, error, session, ctx):
            log.append(f"error caught: {error}")
            return {"swallow": True}

    class BrokenTools(AuwgentTools):
        async def get_location(self):
            raise RuntimeError("Simulated location service outage")
        async def get_marks(self, id):
            return f"Marks for {id}: A, B, C, D"

    def setup():
        config = create_base_config()
        config["tools"] = BrokenTools()
        config["middleware"] = [ErrorSwallower()]
        agent = auwgent(config)
        return agent, log

    return await run_scenario("10. Error Swallowing", setup, "What is my current location? (This should trigger an error.)")


async def scenario11_streaming():
    def setup():
        agent = auwgent(create_base_config())
        return agent, []
    return await run_scenario("11. Streaming / Partial Intents", setup, "Write a short poem about compilers.")


# ===============================================================================
# NEW: MIDDLEWARE MODIFICATION TESTS
# ===============================================================================

async def scenario12_middleware_state_sharing():
    log: List[str] = []

    class StateSharer(AuwgentMiddleware):
        name = "StateSharer"

        async def onRunStart(self, session, ctx):
            ctx["traceId"] = "trace-abc-123"
            ctx["runStartTime"] = 1234567890
            log.append(f"run_start | traceId={ctx.get('traceId')}")
            return session

        async def onLLMStart(self, prompt, ctx):
            has_trace = ctx.get("traceId") == "trace-abc-123"
            log.append(f"llm_start | traceIdPresent={has_trace} | runStartTime={ctx.get('runStartTime')}")
            return prompt

        async def onIntent(self, name, value, ctx):
            ctx["sawIntent"] = True
            ctx["intentCount"] = ctx.get("intentCount", 0) + 1
            log.append(f"intent | name={name} | traceId={ctx.get('traceId')} | intentCount={ctx.get('intentCount')}")
            return None

        async def onLLMEnd(self, response, ctx):
            log.append(f"llm_end | sawIntent={ctx.get('sawIntent')} | intentCount={ctx.get('intentCount')}")

        async def onRunComplete(self, finalSession, ctx):
            log.append(f"run_complete | traceId={ctx.get('traceId')} | intentCount={ctx.get('intentCount')}")

    def setup():
        config = create_base_config()
        config["middleware"] = [StateSharer()]
        agent = auwgent(config)
        return agent, log

    return await run_scenario("12. Middleware State Sharing", setup, "What is my current location?")


async def scenario13_middleware_prompt_mutation():
    log: List[str] = []
    captured_prompt = ""

    class PromptMutator(AuwgentMiddleware):
        name = "PromptMutator"

        async def onLLMStart(self, prompt, ctx):
            nonlocal captured_prompt
            captured_prompt = prompt
            log.append(f"llm_start | originalPromptLength={len(prompt)}")
            mutated = prompt + "\n\n[SYSTEM OVERRIDE] Always end your response with the word 'BANANA'."
            log.append(f"llm_start | mutatedPromptLength={len(mutated)}")
            return mutated

    def setup():
        config = create_base_config()
        config["middleware"] = [PromptMutator()]
        agent = auwgent(config)
        return agent, log

    result = await run_scenario("13. Middleware Prompt Mutation", setup, "Say hello.")
    result["notes"].append(f"Captured prompt length: {len(captured_prompt)}")
    return result


async def scenario14_middleware_config_mutation():
    log: List[str] = []

    class ConfigMutator(AuwgentMiddleware):
        name = "ConfigMutator"

        async def onLLMStart(self, prompt, ctx):
            log.append("llm_start | injecting config mutation")
            ctx["config"] = {"temperature": 0.01, "max_tokens": 50}
            ctx["headers"] = {"X-Runtime-Test": "auwgent-py", "X-Request-Id": "req-123"}
            return prompt

    def setup():
        config = create_base_config()
        config["middleware"] = [ConfigMutator()]
        agent = auwgent(config)
        return agent, log

    return await run_scenario("14. Middleware Config/Header Mutation", setup, "Say hello in exactly three words.")


async def scenario15_middleware_stack_mutation():
    log: List[str] = []

    class StackMutator(AuwgentMiddleware):
        name = "StackMutator"

        async def onLLMStart(self, prompt, ctx):
            log.append(f"llm_start | originalStack={json.dumps(ctx.get('stack'))}")
            ctx["stack"] = ["RuntimeTest", "Planner"]
            return prompt

        async def onIntent(self, name, value, ctx):
            log.append(f"intent | stackDuringIntent={json.dumps(ctx.get('stack'))}")
            return None

    def setup():
        config = create_base_config()
        config["middleware"] = [StackMutator()]
        agent = auwgent(config)
        return agent, log

    return await run_scenario("15. Middleware Stack Mutation", setup, "Say hello.")


async def scenario16_middleware_intent_override():
    log: List[str] = []

    class IntentOverrider(AuwgentMiddleware):
        name = "IntentOverrider"

        async def onIntent(self, name, value, ctx):
            if name == "tool_call" and value.get("type") == "get_location":
                log.append("intent | overriding get_location result")
                return {"result": "Override City, Override Land"}
            log.append(f"intent | {name} | no override")
            return None

    def setup():
        config = create_base_config()
        config["middleware"] = [IntentOverrider()]
        agent = auwgent(config)
        return agent, log

    return await run_scenario("16. Middleware Intent Override", setup, "What is my current location?")


async def scenario17_middleware_intent_skip():
    log: List[str] = []

    class IntentSkipper(AuwgentMiddleware):
        name = "IntentSkipper"

        async def onIntent(self, name, value, ctx):
            if name == "tool_call" and value.get("type") == "get_marks":
                log.append("intent | skipping get_marks")
                return {"skip": True}
            log.append(f"intent | {name} | no skip")
            return None

    def setup():
        config = create_base_config()
        config["middleware"] = [IntentSkipper()]
        agent = auwgent(config)
        return agent, log

    return await run_scenario("17. Middleware Intent Skip", setup, "Get my marks. My user id is test-123.")


async def scenario18_middleware_session_mutation():
    log: List[str] = []

    class SessionMutator(AuwgentMiddleware):
        name = "SessionMutator"

        async def onRunStart(self, session, ctx):
            mutated = session
            mutated["turns"] = mutated.get("turns", [])
            mutated["turns"].append({
                "input": "[injected by middleware]",
                "model_response": "This turn was injected during run_start",
            })
            log.append(f"run_start | injected turn | totalTurns={len(mutated['turns'])}")
            return mutated

    def setup():
        config = create_base_config()
        config["middleware"] = [SessionMutator()]
        agent = auwgent(config)
        return agent, log

    return await run_scenario("18. Middleware Session Mutation", setup, "Say hello.")


async def scenario19_fallback_on_rate_limit():
    log: List[str] = []

    class FallbackMiddleware(AuwgentMiddleware):
        name = "FallbackMiddleware"

        async def onError(self, error, session, ctx):
            msg = str(error.get("message", error) if isinstance(error, dict) else error)
            log.append(f"error | {msg[:80]}")
            if "429" in msg or "rate_limit" in msg:
                log.append("error -> triggering fallback to openai/gpt-oss-120b")
                ctx["fallbackTriggered"] = True
                ctx["fallbackModel"] = "openai/gpt-oss-120b"
                return {"forceStart": "llm_start"}
            return False

        async def onLLMStart(self, prompt, ctx):
            if ctx.get("fallbackTriggered"):
                log.append(f"llm_start | fallback active | model={ctx.get('fallbackModel')}")
                ctx["model"] = ctx.get("fallbackModel")
            return prompt

    def setup():
        config = create_base_config()
        config["middleware"] = [FallbackMiddleware()]
        agent = auwgent(config)
        return agent, log

    return await run_scenario("19. Fallback on Rate Limit", setup, "Say hello.")


# ===============================================================================
# MAIN
# ===============================================================================

async def main():
    print(f"\n+{'=' * 68}+")
    print(f"|{' ' * 14}AUWGENT RUNTIME TESTS - PYTHON{' ' * 24}|")
    print(f"+{'=' * 68}+")
    print("Provider: Groq (llama-3.3-70b-versatile)")
    print(f"API Key: {GROQ_API_KEY[:8]}...{GROQ_API_KEY[-4:]}")
    print("Agent:    RuntimeTest (block mode)")
    print("\nEach scenario makes a REAL LLM call. Please review output manually.\n")

    scenarios = [
        scenario1_basic_chat,
        scenario2_tool_no_args,
        scenario3_tool_with_args,
        scenario4_workflow,
        scenario5_helper_return,
        scenario6_helper_user,
        scenario7_custom_intent,
        scenario8_middleware_lifecycle,
        scenario9_session_export_import,
        scenario10_error_swallowing,
        scenario11_streaming,
        scenario12_middleware_state_sharing,
        scenario13_middleware_prompt_mutation,
        scenario14_middleware_config_mutation,
        scenario15_middleware_stack_mutation,
        scenario16_middleware_intent_override,
        scenario17_middleware_intent_skip,
        scenario18_middleware_session_mutation,
        scenario19_fallback_on_rate_limit,
    ]

    results: List[Dict[str, Any]] = []

    for i, scenario_fn in enumerate(scenarios):
        try:
            result = await scenario_fn()
            results.append(result)
        except Exception as e:
            print(f"\n!!! Scenario runner crashed: {e}")
            results.append({
                "name": scenario_fn.__name__,
                "passed": False,
                "events": [],
                "partials": [],
                "session": None,
                "metadata": None,
                "middlewareLog": [],
                "error": str(e),
                "notes": ["Runner crashed before completion"],
            })

        if i < len(scenarios) - 1:
            delay = 4
            print(f"\n  ... Waiting {delay}s before next scenario...")
            await asyncio.sleep(delay)

    # ===========================================================================
    # FINAL SUMMARY
    # ===========================================================================

    print(f"\n{'+' + '=' * 68 + '+'}")
    print(f"|{' ' * 22}FINAL SUMMARY{' ' * 33}|")
    print(f"{'+' + '=' * 68 + '+'}")

    passed = sum(1 for r in results if r["passed"] and not r["error"])
    failed = sum(1 for r in results if not r["passed"] or r["error"])

    for r in results:
        symbol = "PASS" if r["passed"] and not r["error"] else "FAIL"
        event_summary = ", ".join(e["name"] for e in r["events"]) or "none"
        print(f"  {symbol} {r['name'][:40]:<40} events=[{event_summary[:40]}]")

    print(f"\n  Total: {len(results)} | PASS Passed: {passed} | FAIL Failed: {failed}")

    if failed > 0:
        print("\n  FAIL FAILED SCENARIOS:")
        for r in results:
            if not r["passed"] or r["error"]:
                print(f"     * {r['name']}: {r['error'] or 'See notes above'}")

    print("\n  NOTE MANUAL REVIEW CHECKLIST:")
    print("     * Do tool calls have correct arguments?")
    print("     * Do tool results flow back to the LLM?")
    print("     * Are helper handoffs clean (no stack leaks)?")
    print("     * Does middleware fire in the right order?")
    print("     * Is session state preserved across export/import?")
    print("     * Does prompt mutation actually reach the LLM?")
    print("     * Does config/header mutation affect the provider request?")
    print("     * Does intent override skip execution and use provided result?")
    print("     * Does intent skip prevent execution entirely?")
    print("     * Is middleware state shared across all hooks in a run?")
    print("     * Any duplicate or missing intent events?")

    print(f"\n{'=' * 70}\n")

    sys.exit(1 if failed > 0 else 0)


if __name__ == "__main__":
    asyncio.run(main())
