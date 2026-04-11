import 'main.agent.dart';

final class HelloLogger extends AuwgentBaseIntentHandler {
  @override
  Object? responseText(intent, agentName) {
    print('text intent from $agentName');
    print(intent.text);
    return null;
  }
  @override
  Object? responseSchema(intent, agentName) {
    print('schema intent from $agentName');
    print(intent.response);
    return null;
  }

  @override
  Object? toolCall(intent,agentName){
    print(" called ,${intent.args}, name $agentName");  
    return null;
  }

   @override
  Object? toolResult(intent,agentName){
    print(" result, ${intent.args}, name $agentName");  
    return null;
  }
}

final class Tools extends AuwgentTools {
  const Tools();

  @override
  Future<String> getDetails() async => "Theo, 24";

  @override
  Future<String> getLocation(args) async => "Lagos ${args.id}";
}


Future<void> main() async {


  final config = AuwgentConfig( 
    apiKeys: AuwgentApiKeys(
      geminiApiKey: 'AIzaSyCGodWJEMHYyPKzume13PXo6dez45W3SCY',
    ),
    tools: Tools()
  );

  final agent = auwgent(config);
  print(agent.generatePrompt());

  agent.onIntentHandler(HelloLogger());

  // agent.onIntent((name, value, agentName) {
  //   print('intent: $name');
  //   print('agent: $agentName');
  //   print('value: $value');
  //   return null;
  // });

  try {

    final session = await agent.run('Hello there what my name and age');
    print(session.turns.toString());
  } finally {
    agent.dispose();
  }
}
