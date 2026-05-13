// Auto-generated Dart IR for RuntimeTest
// Do not edit manually
import 'dart:convert';
import 'package:auwgent_sdk_dart/auwgent.dart' as sdk;

const String _RuntimeTestAgentIrJson = r'''{
  "name": "RuntimeTest",
  "modelConfig": [
    {
      "defaultConfig": {
        "model": {
          "type": "groq",
          "modelName": "llama-3.3-70b-versatile",
          "config": null
        },
        "embedding": null,
        "prompt": {
          "type": "template",
          "value": [
            {
              "type": "literal",
              "value": "\n        You are a helpful test assistant. You have access to tools and helpers.\n        Use tools when the user asks for data. Use helpers for specialized tasks.\n        When you need to think out loud, use the [custom:Loud] intent.\n        Always respond using the correct protocol blocks.\n        "
            }
          ]
        },
        "toolProtocol": "block"
      },
      "namedConfig": []
    }
  ],
  "input": null,
  "output": null,
  "context": {
    "user_name": {
      "type": "string",
      "optional": false
    },
    "age": {
      "type": "number",
      "optional": false
    },
    "id": {
      "type": "string",
      "optional": false
    }
  },
  "tools": [
    {
      "name": "get_location",
      "description": "Return the current location for the active user",
      "params": {},
      "returns": "string",
      "examples": []
    },
    {
      "name": "get_marks",
      "description": "Return the user's score",
      "params": {
        "id": {
          "type": "string",
          "optional": false,
          "description": "the id of the user"
        }
      },
      "returns": "string",
      "examples": []
    }
  ],
  "workflows": [
    {
      "flowName": "marks_and_location",
      "flowParams": {
        "user_id": {
          "type": "string",
          "optional": false
        }
      },
      "returns": "string",
      "description": "Get both the location and marks of a user in one go",
      "body": [
        {
          "type": "variableDeclaration",
          "name": "marks",
          "value": {
            "type": "functionCall",
            "value": "get_marks",
            "args": [
              {
                "type": "varRef",
                "value": "user_id"
              }
            ]
          }
        },
        {
          "type": "variableDeclaration",
          "name": "location",
          "value": {
            "type": "functionCall",
            "value": "get_location",
            "args": []
          }
        },
        {
          "type": "return",
          "value": {
            "type": "template",
            "value": [
              {
                "type": "literal",
                "value": "\n            Location: "
              },
              {
                "type": "varRef",
                "value": "location"
              },
              {
                "type": "literal",
                "value": "\n            Marks: "
              },
              {
                "type": "varRef",
                "value": "marks"
              },
              {
                "type": "literal",
                "value": "\n        "
              }
            ]
          }
        }
      ],
      "tools": [],
      "examples": [
        {
          "user_id": {
            "type": "literal",
            "value": "100"
          }
        }
      ]
    }
  ],
  "helpers": [
    {
      "name": "Planner",
      "description": "A planning specialist that breaks down tasks into steps",
      "modelConfig": [
        {
          "defaultConfig": {
            "model": {
              "type": "groq",
              "modelName": "llama-3.3-70b-versatile",
              "config": null
            },
            "embedding": null,
            "prompt": {
              "value": "You are a task planner. Break down the user's request into clear steps.",
              "type": "literal"
            },
            "toolProtocol": "block"
          },
          "namedConfig": []
        }
      ],
      "input": null,
      "output": {
        "type": {
          "type": "object",
          "properties": {
            "steps": {
              "type": {
                "type": "array",
                "items": "string"
              },
              "optional": false,
              "description": "Step-by-step plan"
            },
            "motivation": {
              "type": "string",
              "optional": false,
              "description": "Why this plan is the right approach"
            }
          }
        }
      },
      "context": null,
      "tools": [],
      "workflows": [],
      "customIntents": null,
      "examples": []
    },
    {
      "name": "Joker",
      "description": "A joke-telling helper that makes people laugh",
      "modelConfig": [
        {
          "defaultConfig": {
            "model": {
              "type": "groq",
              "modelName": "llama-3.3-70b-versatile",
              "config": null
            },
            "embedding": null,
            "prompt": {
              "value": "You are a comedian. Tell a joke based on the user's request.",
              "type": "literal"
            },
            "toolProtocol": "block"
          },
          "namedConfig": []
        }
      ],
      "input": null,
      "output": null,
      "context": null,
      "tools": [],
      "workflows": [],
      "customIntents": null,
      "examples": []
    }
  ],
  "components": [],
  "types": null,
  "helperToolGrants": null,
  "helperHandoff": {
    "Joker": "user"
  },
  "tests": [],
  "lifecycle": null,
  "customIntents": [
    {
      "name": "Loud",
      "description": "Use this to explain your thought process and actions out loud",
      "fields": {
        "actions": {
          "type": "string",
          "optional": false,
          "description": "The action you are about to take"
        },
        "reason": {
          "type": "string",
          "optional": false,
          "description": "Why you are taking this action"
        }
      },
      "examples": [
        {
          "actions": {
            "type": "literal",
            "value": "I will look up the user's location"
          },
          "reason": {
            "type": "literal",
            "value": "The user asked where they are"
          }
        }
      ]
    }
  ]
}''';

sdk.JsonMap decodeRuntimeTestIr() {
  return Map<String, Object?>.from(jsonDecode(_RuntimeTestAgentIrJson) as Map);
}
