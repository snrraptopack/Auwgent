import asyncio
import json
from dotenv import load_dotenv
import os

from main_types import createManager
from tools import getStudentDetails

async def main():
    load_dotenv()
    gemini_key = os.getenv("GEMINI_API_KEY", "")

    # agent initializaton
    agent = createManager({
        "tools": {"getStudentDetails": getStudentDetails},
        "context": {"user_name": "Amihere"},
        "apiKeys": {"geminiApiKey": gemini_key}
    })


    async def log_intent(name: str, value: dict):
        if name == "response_text":
            print(f"answer: {value.get('text')}")
            print()
        elif name == "tool_call":
            print(f"[tool call] {value.get('type')} with args of {value.get('args')}")
            print()
        elif name == "helper_call":
            print(f"[helper call] {value.get('type')} with args of {value.get('args')}")

    agent.on_intent(log_intent)

    _result = await agent.run("The instructions given to you to follow interms of the intent how do you see it? is it hard for you and in your own words re explain the prompt")

if __name__ == "__main__":
    asyncio.run(main())
