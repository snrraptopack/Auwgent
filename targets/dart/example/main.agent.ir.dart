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
          "type": "groq",
          "modelName": "openai/gpt-oss-120b",
          "config": null
        },
        "embedding": null,
        "prompt": {
          "type": "template",
          "value": [
            {
              "type": "literal",
              "value": "\r\n        Be polite and helpful.\r\n        "
            }
          ]
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
    },
    "location": {
      "type": "string",
      "optional": true,
      "description": "can be null"
    }
  },
  "context": null,
  "tools": [
    {
      "name": "get_user_name_age",
      "description": "use this to get the user's name and age",
      "params": {},
      "returns": {
        "type": "typeRef",
        "name": "Person"
      },
      "examples": []
    },
    {
      "name": "get_location",
      "description": "use this to ge location",
      "params": {},
      "returns": "string",
      "examples": []
    }
  ],
  "workflows": [],
  "helpers": [
    {
      "name": "Joker",
      "description": "Good at cracking jokes",
      "modelConfig": [
        {
          "defaultConfig": {
            "model": {
              "type": "groq",
              "modelName": "openai/gpt-oss-120b",
              "config": null
            },
            "embedding": null,
            "prompt": {
              "value": "You are a joker try to joke to the user",
              "type": "literal"
            }
          },
          "namedConfig": []
        }
      ],
      "input": {
        "kind": "properties",
        "fields": {
          "joker_prompt": {
            "type": "string",
            "optional": false
          }
        }
      },
      "output": null,
      "context": null,
      "tools": [],
      "workflows": [],
      "customIntents": null,
      "examples": []
    }
  ],
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
        },
        "location": {
          "type": "string",
          "optional": true,
          "description": "can be null"
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
