import asyncio
from generated.main_types import (
    AuwgentBasePartialIntentHandler,
    auwgent,
    AuwgentConfig,
    AuwgentContext,
    AuwgentMiddleware,
    MainIntentName,
)

class Logger(AuwgentMiddleware):
    name= "simple"

    async def onRunComplete(self, finalSession, ctx):
       print("")

context:AuwgentContext = {
    'user_id':"",
    'is_vip':True,
    'session_id':""
}

config:AuwgentConfig = {
    'apiKeys':{
        'my_groq_apiApiKey':""
    },
    'context':context,
    'middleware':[Logger]
}

agent = auwgent(config)

class PartialHander(AuwgentBasePartialIntentHandler):
    def response_text(self, intent, agent_name: str):
        print(intent.get('delta'))

    def response_schema(self,intent,agent_name):




agent.on_intent_partial(PartialHander)

print(agent.generate_prompt())

async def main():
    session = await agent.run("hello")
    print(agent.get_metadata())

asyncio.run(main())
