import 'dart:async';

import 'main.agent.dart';

final class ML extends AuwgentMiddleware {
  onRunStart(session, ctx) {
    print("The context $ctx");
    return session;
  }
}


final class HelloLogger extends AuwgentBaseIntentHandler {
  responseText(value, agentName) {
    print("text");
    print("${value.text} by $agentName");
  }

  responseSchema(value, agentName) {
    print("schema");
    print("${value.response} by $agentName ");
  }

  toolCall(value, agentName) {
    print(" tool called: $agentName ,${value}");
    return null;
  }

  toolResult(value, agentName) {
    print(" tool result name $agentName , ${value}");
  }

}

final class Tools extends AuwgentTools {
  const Tools();

  @override
  Future<Person> getUserNameAge() async => Person(age: 10, name: "ama");

  @override
  Future<String> getLocation() async => "Tarkwa";
}

Future<void> main() async {
  final config = AuwgentConfig(
    apiKeys: AuwgentApiKeys(
      groqApiKey: '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',
    ),
    tools: Tools(),
    middleware: [ML()],
  );

  final agent = auwgent(config);

  //print(agent.generatePrompt());

  agent.onIntentHandler(HelloLogger());
  //agent.onIntentPartialHandler(Logger());

  // agent.onIntent((name, value, agentName) {
  //   print('intent: $name');
  //   print('agent: $agentName');
  //   print('value: $value');
  //   return null;
  // });

  try {
    final session = await agent.run(
      'Hello get my name and my location',
    );

    print(agent.getMetadata());
    print(session.turns);

  } finally {
    agent.dispose();
  }
}


/** issues
 * 
 * The context {
  "activeAgent": "Hello",
  "stack": [],
  "rootAgent": "Hello",
  "rawBlock": null,
  "systemPrompt": null
}
{
  "aggregate": {
    "prompt_tokens": 329,
    "completion_tokens": 28,
    "total_tokens": 357,
    "reasoning_tokens": 0,
    "cached_tokens": 0
  },
  "turns": [
    {
      "turn_index": 0,
      "usage": {
        "prompt_tokens": 329,
        "completion_tokens": 28,
        "total_tokens": 357,
        "reasoning_tokens": 0,
        "cached_tokens": 0
      },
      "finish_reason": "stop",
      "model": "llama-3.3-70b-versatile"
    }
  ]
}
[{
  "input": "Hello get my name and my location",
  "model_response": " \n[tool_call: get_user_name_age] \n[/tool_call]\n[tool_call: get_location] \n[/tool_call]"
}]
PS C:\Users\babyface\Desktop\auwgent\Auwgent\targets\dart\example> 


 * 
 */


/**
 *
 *   "systemPrompt": null
}
called helper {
  "joker_prompt": "Tell me a joke"
} by Hello
Why don&#39;t scientists trust atoms? Because they make up everything!
Why don&#39;t scientists trust atoms? Because they make up everything! by Hello
{
  "aggregate": {
    "prompt_tokens": 816,
    "completion_tokens": 323,
    "total_tokens": 1139
  },
  "turns": [
    {
      "turn_index": 0,
      "usage": {
        "prompt_tokens": 370,
        "completion_tokens": 112,
        "total_tokens": 482
      },
      "finish_reason": "stop",
      "model": "openai/gpt-oss-120b"
    },
    {
      "turn_index": 1,
      "usage": {
        "prompt_tokens": 446,
        "completion_tokens": 211,
        "total_tokens": 657
      },
      "finish_reason": "stop",
      "model": "openai/gpt-oss-120b"
    }
  ]
}
[{
  "input": "Hello please tell me a joke",
  "model_response": "[helper_call: Joker]\njoker_prompt: Tell me a joke\n[/helper]"
}, {
  "input": "[result]\nname: helper:Joker\nargs:\n  joker_prompt: Tell me a joke\nresult:\n  result: '[response_text]Why don''t scientists trust atoms? Because they make up everything![/response_text]'\n[/result]",
  "model_response": "[response_text]Why don&#39;t scientists trust atoms? Because they make up everything![/response_text]"
}]
PS C:\Users\babyface\Desktop\auwgent\Auwgent\targets\dart\example>
 */
