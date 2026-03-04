import json
import asyncio
from dotenv import load_dotenv
import os

# Generated types handle all imports (including auwgent SDK)
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
    load_dotenv()
    gemini_key = os.getenv("GEMINI_API_KEY", "")

    agent = createManager({
        "tools": MyManagerTools(),
        "context": {"user_name": "sysadmin"},
        "apiKeys": {"geminiApiKey": gemini_key}
    })

    async def log_intent(name: str, value: dict):
        if name == "response_text":
            print(f"🤖 Agent says: {value.get('text')}")
        elif name == "tool_call":
            print(f"🛠️ Agent is calling tool '{value.get('type')}' with args: {value.get('args')}")
        elif name == "tool_result":
            print(f"✅ Tool '{value.get('name')}' finished.")
        else:
            print(f"[{name.upper()}]: {json.dumps(value)}")

    agent.on_intent(log_intent)

    try:
        result = await agent.run("what is the deatails for student with id 10")
        print(f"output : {json.dumps(result)}")
    except Exception as e:
        print(f"Engine Exception: {e}")

if __name__ == "__main__":
    asyncio.run(main())
