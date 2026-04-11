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
      "optional": false
    },
    "age": {
      "type": "number",
      "optional": false
    }
  },
  "context": null,
  "tools": [
    {
      "name": "get_details",
      "description": "use this to get the deatails of the user",
      "params": {},
      "returns": "string",
      "examples": []
    },
    {
      "name": "get_location",
      "description": "use this to ge location",
      "params": {
        "id": {
          "type": "string",
          "optional": false
        }
      },
      "returns": "string",
      "examples": []
    }
  ],
  "workflows": [],
  "helpers": [],
  "components": [],
  "types": {
    "Person": {
      "isOutput": false,
      "properties": {
        "name": {
          "type": "string",
          "optional": false,
          "description": null
        },
        "age": {
          "type": "number",
          "optional": false,
          "description": null
        }
      },
      "@examples": []
    }
  },
  "helperToolGrants": null,
  "helperHandoff": null,
  "tests": [],
  "lifecycle": null,
  "customIntents": null
}''';

sdk.JsonMap decodeHelloIr() {
  return Map<String, Object?>.from(jsonDecode(_HelloAgentIrJson) as Map);
}
