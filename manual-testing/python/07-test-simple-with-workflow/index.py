import asyncio
from generated.main_types import (
    AuwgentMiddleware,
    MainIntentValue,
    auwgent,
    AuwgentConfig,
    AuwgentTools,
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

def handle_partial(intent:MainIntentName, data:MainIntentValue, session_id):
    if intent == "response_text":
        print(data.get("delta",""), end="")

agent.on_intent_partial(handle_partial)

print(agent.generate_prompt())

async def main():
    session = await agent.run("hello")
    print(agent.get_metadata())

asyncio.run(main())
