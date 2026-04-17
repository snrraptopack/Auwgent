import 'main.agent.dart';
import "dart:io";

final class ML extends AuwgentMiddleware {
  onRunStart(session, ctx) {
    print("The context $ctx");
    return session;
  }
}

final class Logger extends AuwgentBasePartialIntentHandler {
  responseText(intent, name) {
    stdout.write(intent.delta ?? "");
  }

  responseSchema(value, name) {
    
  }
}

final class HelloLogger extends AuwgentBaseIntentHandler {
  responseText(intent, agentName) {
    print(intent.text);
    print("${intent.text} by $agentName");
  }

  responseSchema(intent, agentName) {
    print('schema intent from $agentName');
    print("${intent.response} by ");
  }

  toolCall(intent, agentName) {
    print(" called ,${intent.args}, name $agentName");
    return null;
  }

  toolResult(intent, agentName) {
    print(" result, ${intent.args}, name $agentName");
  }

  helperCall(intent, name) {
    print("called helper ${intent.args} by $name");
  }

  helperResult(intent, name) {
    print("result ${intent.result}, by $name");
  }
}

final class Tools extends AuwgentTools {
  const Tools();

  @override
  Future<Person> getDetails() async => Person(age: 10, name: "ama");

  @override
  Future<String> getLocation(args) async => "Tarkwa ${args.id}";
}

Future<void> main() async {
  final config = AuwgentConfig(
    apiKeys: AuwgentApiKeys(
      groqApiKey:
          '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',
    ),
    tools: Tools(),
    middleware: [ML()],
  );

  final agent = auwgent(config);

  print(agent.generatePrompt());

  //agent.onIntentHandler(HelloLogger());
  agent.onIntentPartialHandler(Logger());

  // agent.onIntent((name, value, agentName) {
  //   print('intent: $name');
  //   print('agent: $agentName');
  //   print('value: $value');
  //   return null;
  // });

  try {
    final session = await agent.run('Hello get my name and my location with id 10');

    print(agent.getMetadata());

    print("${session.turns}");
  } finally {
    agent.dispose();
  }
}

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
