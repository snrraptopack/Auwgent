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
    AuwgentTools,
    User
)


class Tools(AuwgentTools):
    async def sum_order_totals(self, *, orders_json: str):
       return {'skip':True}

    async def remove_user(self,id):
        # your implementation here
        return f"deleted user {id}"


class ResearcherMiddleware(AuwgentMiddleware):
    name = "researcher-middleware"
    target = "Researcher"  # only fires when Researcher helper is active

    async def onLLMStart(self, prompt: str, ctx):
        # ctx["activeAgent"] is guaranteed to be "Researcher" here
        pass

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
    'middleware':[Logger],
    'tools':Tools
}



class HandleIntent(AuwgentBaseIntentHandler):
    async def workflow_call(self, intent, agent_name: str):
        print("workflow triggered:", intent.get("type"))

    async def workflow_result(self, intent, agent_name: str):
        print("workflow result:", intent)

agent.on_intent(HandleIntent)

print(agent.generate_prompt())
