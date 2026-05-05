import asyncio
import json
from dotenv import load_dotenv
import os

from main_types import AuwgentContext, AuwgentMiddleware, AuwgentApiKeys, auwgent,AuwgentBaseIntentHandler,AuwgentConfig
from tools import Tools


class MLogger(AuwgentMiddleware):
    name = "simple loggler"

    async def onRunStart(self, session, ctx):
        ctx["one"] = "coming from one"
        print("middleware needs to log")
        return session


    async def onLLMStart(self, prompt, ctx):
        simple = "my name is Theophilus Amihere Junior I come from Ghana i am 10" + prompt

        print(f"context is {ctx['one']}")
        return simple


class Logger(AuwgentBaseIntentHandler):

    def response_text(self, value, agent_name):
        print(f"answer: {value.get('text')}")
        print()

    def response_schema(self, value, agent_name):
        print(f"schema: {json.dumps(value['response'], indent=2)}")
        print()

    def tool_call(self, value, agent_name: str):

        if value['type'] == 'get_marks':
            args = value.get('args')

        print(f"[tool call] {json.dumps(value,indent=2)}")
        print()

    def tool_result(self, value, agent_name: str):
        print(f"[tool result] {json.dumps(value, indent=2)}")
        print()

    def error(self, value, agent_name: str):
        print(value['message'])


load_dotenv()
groq_key = os.getenv("GROQ_API_KEY", "")

config = AuwgentConfig(
    apiKeys=AuwgentApiKeys(groqApiKey=groq_key),
    tools=Tools(),
     middleware=[MLogger],
    context= AuwgentContext(
        age=10,
        user_name= "Amihere",
        id="100"
    )
)

async def main():

    print("before auwgent config")

    # agent initializaton
    agent = auwgent(config)

    agent.on_intent(Logger)

    #print(agent.generate_prompt())

    _result = await agent.run("Hello get my marks and my location and by the how are you?")
    print(json.dumps(_result['turns']))

if __name__ == "__main__":
    asyncio.run(main())

# "model_response": " \n[tool_call: get_user_name_age]\n[/tool_call]\n[tool_call: get_location]\n[/tool_call]"
