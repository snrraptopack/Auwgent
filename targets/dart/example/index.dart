import 'main.agent.dart';


final class ML extends AuwgentMiddleware{
  

}

final class Logger extends AuwgentBasePartialIntentHandler{

  @override
  Object? responseText(intent,name){
    return null;
  }

  
  responseSchema(intent,name){
    return null;
  }
}

final class HelloLogger extends AuwgentBaseIntentHandler {
  responseText(intent, agentName) {
    print(intent.text);
    return null;
  }
  
  responseSchema(intent, agentName) {
    print('schema intent from $agentName');
    print(intent.response);
    return null;
  }

  toolCall(intent,agentName){
    print(" called ,${intent.args}, name $agentName");
    return null;
  }

  toolResult(intent,agentName){
    print(" result, ${intent.args}, name $agentName");
    return null;
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

    final session = await agent.run('Hello there what my name and age and my location');

    print(agent.getMetadata());

    session.turns.map((s)=>{
      print(" user input ${s.input}  model output ${s.modelResponse}")
    });

  } finally {
    agent.dispose();
  }
}
