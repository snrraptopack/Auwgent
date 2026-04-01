import asyncio

from typing_extensions import override
from generated.main_types import (
    AuwgentBaseIntentHandler,
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
       ctx['set_context']({'name':10})



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


class IntentHandler(AuwgentBaseIntentHandler):
