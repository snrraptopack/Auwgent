import 'main.agent.dart';

final class HelloLogger extends AuwgentBaseIntentHandler {
  @override
  Object? responseText(ResponseText intent, String agentName) {
    print('text intent from $agentName');
    print(intent.text);
    return null;
  }


   @override
  Object ? responseSchema(ResponseSchema intent, String agentName) {
    final output = intent.response;

    return null;
  }

}

Future<void> main() async {
  final agent = createHello(
    const HelloConfig(
      apiKeys: HelloApiKeys(
        geminiApiKey: '..',
      ),
    ),
  );


  print(agent.generatePrompt());

  agent.onIntentHandler(HelloLogger());

  agent.onIntent((name, value, agentName) {
    print('intent: $name');
    print('agent: $agentName');
    print('value: $value');
    return null;
  });

  try {

    final session = await agent.run('Hello there');
    print(session.turns.toString());
  } finally {
    agent.dispose();
  }
}
