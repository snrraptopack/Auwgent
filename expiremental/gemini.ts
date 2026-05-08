const GEMINI_API_KEY = "AIzaSyCGodWJEMHYyPKzume13PXo6dez45W3SCY"

const url = `https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent?key=${GEMINI_API_KEY}`

const body = {
  contents: [
    {
      role: "user",
      parts: [
        {
          text: "What's in this image?"
        },
        {
          file_data: {
            mime_type: "image/jpeg",
            file_uri: "https://upload.wikimedia.org/wikipedia/commons/f/f2/LPU-v1-die.jpg"
          }
        }
      ]
    }
  ]
}

const method = 'POST'
const b = JSON.stringify(body)
const headers = {
  'Content-Type': 'application/json'
}

const req = await fetch(url, { method, headers, body: b });
const response = await req.json()

// console.log(JSON.stringify(response, null, 2))

// Copyright (C) Microsoft Corporation. All rights reserved.

// Try the new cross-platform PowerShell https://aka.ms/pscore6

// PS C:\Users\babyface\Desktop\auwgent\Auwgent> cd expiremental
// PS C:\Users\babyface\Desktop\auwgent\Auwgent\expiremental> bun run gemini
// {
//   "candidates": [
//     {
//       "content": {
//         "parts": [
//           {
//             "text": "This image appears to be a microscopic photograph or a highly detailed rendering of a semiconductor chip, likely a Central Processing Unit (CPU) or Graphics Processing Unit (GPU) die.\n\nHere are some details I can observe:\n\n*   **Intricate Patterning:** The entire surface is covered in extremely fine, repetitive patterns, which are characteristic of integrated circuits.\n*   **Regular Grid Structures:** There are large, very organized grid-like areas, which likely correspond to memory arrays (like cache) or processing cores. The dark rectangular areas within the grid would be individual transistors or cells.\n*   **Interconnects:** Fainter lines and pathways connecting different sections are visible, representing the metal layers that form the electrical interconnects between the various components on the chip.\n*   **Peripheral Structures:** Along the edges, particularly the top, bottom, and sides, there are areas with different, often more varied, patterns. These could be I/O (Input/Output) pads for connecting to the outside world, control logic, power delivery networks, or other specialized circuitry.\n*   **Color Palette:** The image has a warm, metallic, sepia-toned appearance, which is common in optical microscope images of silicon wafers or chip dies due to the materials and lighting used.\n*   **Scale:** The sheer density of components indicates a microscopic scale, where each visible square or line represents thousands or millions of transistors.\n\nIn essence, it's a \"city plan\" of a very complex electronic brain!"
//           }
//         ],
//         "role": "model"
//       },
//       "finishReason": "STOP",
//       "index": 0
//     }
//   ],
//   "usageMetadata": {
//     "promptTokenCount": 266,
//     "candidatesTokenCount": 310,
//     "totalTokenCount": 576,
//     "promptTokensDetails": [
//       {
//         "modality": "TEXT",
//         "tokenCount": 8
//       },
//       {
//         "modality": "IMAGE",
//         "tokenCount": 258
//       }
//     ],
//     "serviceTier": "standard"
//   },
//   "modelVersion": "gemini-2.5-flash-image",
//   "responseId": "gkP9acCIAeuIvdIP7PmywAs"
// }
// PS C:\Users\babyface\Desktop\auwgent\Auwgent\expiremental>
