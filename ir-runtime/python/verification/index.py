import json
import os
import asyncio
from dotenv import load_dotenv

# Ensure parent directory is in path to import auwgent.py
import sys
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))

# We can import the generated types for static typing
from main_types import Student, ManagerTools, createManager

class MyManagerTools:
    async def getStudentDetails(self, id: str) -> Student:
        return {
            "user_name": "Babyface",
            "age": 22,
            "id": id,
            "grades": ["A", "A+"]
        }

async def main():

    # Load the .env file
    load_dotenv()
    gemini_key = os.getenv("GEMINI_API_KEY", "")

    tools = MyManagerTools()
    config = {
        "tools": tools,
        "context": {"user_name": "sysadmin"},
        "apiKeys": {"geminiApiKey": gemini_key}
    }

    # Initialize the engine natively via generated factory
    agent = createManager(config)

    # Listen to intent streams in real-time
    async def log_intent(name: str, value: dict):
        # We can selectively log intents to the console as they happen
        if name == "response_text":
            print(f"🤖 Agent says: {value.get('text')}")
        elif name == "tool_call":
            print(f"🛠️ Agent is calling tool '{value.get('type')}' with args: {value.get('args')}")
        elif name == "tool_result":
            print(f"✅ Tool '{value.get('name')}' finished.")
        else:
            print(f"[{name.upper()}]: {json.dumps(value)}")

    agent.on_intent(log_intent)

    # Run the agent with a basic input
    try:
        _results = await agent.run("what is the deatails for student with id 10")

        # print("\nProcessed Output:")
        # print(json.dumps(results, indent=2))

    except Exception as e:
        print(f"Engine Exception: {e}")

if __name__ == "__main__":
    asyncio.run(main())
