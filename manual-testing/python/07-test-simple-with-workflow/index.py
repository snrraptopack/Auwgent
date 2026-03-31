import asyncio
from generated.main_types import (
    AuwgentMiddleware,
    auwgent,
    AuwgentConfig,
    AuwgentTools,
    AuwgentContext,
    AuwgentMiddleware
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

print(agent.generate_prompt())

async def main():
    session = await agent.run("hello")
    print(agent.get_metadata())

asyncio.run(main())
