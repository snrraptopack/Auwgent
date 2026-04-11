import 'main.agent.dart';

final class HelloLogger extends HelloBaseIntentHandler {
  @override
  Object? responseText(ResponseText intent, String agentName) {
    print('text intent from $agentName');
    print(intent.text);
    return null;
  }

  @override
  Object? response_schema(ResponseSchema intent, String agentname){
    intent.response.
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

    final session = agent.run('Hello there');
    print(session.turns.toString());
  } finally {
    agent.dispose();
  }
}
