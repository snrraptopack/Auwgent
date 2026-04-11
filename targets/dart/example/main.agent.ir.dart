// Auto-generated Dart IR for Hello
// Do not edit manually
import 'dart:convert';
import 'package:auwgent_sdk_dart/auwgent.dart' as sdk;

const String _HelloAgentIrJson = r'''{
  "name": "Hello",
  "modelConfig": [
    {
      "defaultConfig": {
        "model": {
          "type": "gemini",
          "modelName": "gemini-2.5-flash",
          "config": null
        },
        "embedding": null,
        "prompt": {
          "value": "You are a helper",
          "type": "literal"
        }
      },
      "namedConfig": []
    }
  ],
  "input": null,
  "output": {
    "name": {
      "type": "string",
      "optional": false,
      "description": "no description"
    },
    "age": {
      "type": "number",
      "optional": false,
      "description": "no description"
    }
  },
  "context": null,
  "tools": [],
  "workflows": [],
  "helpers": [],
  "components": [],
  "types": null,
  "helperToolGrants": null,
  "helperHandoff": null,
  "tests": [],
  "lifecycle": null,
  "customIntents": null
}''';

sdk.JsonMap decodeHelloIr() {
  return Map<String, Object?>.from(jsonDecode(_HelloAgentIrJson) as Map);
}
