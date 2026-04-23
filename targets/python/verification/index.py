import asyncio
import json
from dotenv import load_dotenv
import os
from main_types import AuwgentMiddleware, HelloApiKeys, auwgent,AuwgentBaseIntentHandler,AuwgentConfig
from tools import Tools


class MLogger(AuwgentMiddleware):
    name = "simple loggler"

    async def onRunStart(self, session, ctx):
        print("this logged",json.dumps(session,indent=2))
        return session


    async def onLLMStart(self, prompt, ctx):
        simple = "my name is Theophilus Amihere Junior I come from Ghana i am 10" + prompt
        print(f"sending prompt {simple}")
        print(json.dumps(ctx,indent=2))
        return simple


class Logger(AuwgentBaseIntentHandler):

    def response_text(self, intent, agent_name):
        print(f"answer: {intent.get('text')}")
        print()

    def response_schema(self, intent, agent_name):
        print(f"schema: {json.dumps(intent['response'], indent=2)}")
        print()

    def tool_call(self, intent, agent_name: str):
        print(f"[tool call] {json.dumps(intent,indent=2)}")
        print()

    def tool_result(self, intent, agent_name: str):
        print(f"[tool result] {json.dumps(intent, indent=2)}")
        print()

    def error(self,intent,agent_name):
        print(intent['message'])


load_dotenv()
groq_key = os.getenv("GROQ_API_KEY", "")

config = AuwgentConfig(
    apiKeys=HelloApiKeys(groqApiKey=groq_key),
    tools=Tools(),
    middleware=[MLogger]
)

async def main():

    # agent initializaton
    agent = auwgent(config)


    agent.on_intent(Logger())

    #print(agent.generate_prompt())

    _result = await agent.run("Hello get my name and my location..")
    print(json.dumps(agent.get_metadata(), indent=2))
    #print(json.dumps(_result, indent=2))

if __name__ == "__main__":
    asyncio.run(main())

# "model_response": " \n[tool_call: get_user_name_age]\n[/tool_call]\n[tool_call: get_location]\n[/tool_call]"
