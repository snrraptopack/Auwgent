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
        'my_groq_apiApiKey':"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"
    },
    'context':context
}

agent = auwgent(config)

print(agent.generate_prompt())

async def main():
    session = await agent.run("hello")
    print(agent.get_metadata())

asyncio.run(main())
