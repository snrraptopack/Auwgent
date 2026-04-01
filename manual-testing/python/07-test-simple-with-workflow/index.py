import asyncio
from generated.main_types import (
    AuwgentBasePartialIntentHandler,
    auwgent,
    AuwgentConfig,
    AuwgentContext,
    AuwgentMiddleware,
    MainIntentName,
)

context:AuwgentContext = {
    'user_id':"",
    'is_vip':True,
    'session_id':""
}


config:AuwgentConfig = {
    'apiKeys':{
        'my_groq_apiApiKey':""
    },
    'context':context
}

agent = auwgent(config)

class PartialHander(AuwgentBasePartialIntentHandler):
    def response_text(self, intent, agent_name: str):
        print(intent.get('text'))






agent.on_intent_partial(PartialHander())

print(agent.generate_prompt())

async def main():
    session = await agent.run("hello")
    print(agent.get_metadata())

asyncio.run(main())
