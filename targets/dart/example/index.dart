import 'main.agent.dart';


final class ML extends AuwgentMiddleware{

  onRunStart(session,ctx){
    print("The context $ctx");
    return session;
  }  

}

final class Logger extends AuwgentBasePartialIntentHandler{

  responseText(intent,name){
    return null;
  }

  
  responseSchema(intent,name){
    
  }
}

final class HelloLogger extends AuwgentBaseIntentHandler {
  responseText(intent, agentName) {
    print(intent.text);
  }
  
  responseSchema(intent, agentName) {
    print('schema intent from $agentName');
    print(intent.response);
  }

  toolCall(intent,agentName){
    print(" called ,${intent.args}, name $agentName");
    return null;
  }

  toolResult(intent,agentName){
    print(" result, ${intent.args}, name $agentName");
  }
}

final class Tools extends AuwgentTools {
  const Tools();

  @override
  Future<String> getDetails() async => "Theo, 999";

  @override
  Future<String> getLocation(args) async => "Tarkwa ${args.id}";
}


Future<void> main() async {
  final config = AuwgentConfig(
    apiKeys: AuwgentApiKeys(
      groq_apiApiKey: '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',
    ),
    tools: Tools(),
    middleware: [ML()]
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

    final session = await agent.run('Hello there what my name and age and my location, you can call two tools at the same time and wait for their result');

    print(agent.getMetadata());

    print("${session.turns}");

  } finally {
    agent.dispose();
  }
}
