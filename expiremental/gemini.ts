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

console.log(JSON.stringify(response, null, 2))

// PS C:\Users\babyface\Desktop\auwgent\Auwgent> cd expiremental
// PS C:\Users\babyface\Desktop\auwgent\Auwgent\expiremental> bun run gemini
// {
//   "candidates": [
//     {
//       "content": {
//         "parts": [
//           {
//             "text": "This image appears to be a highly magnified view of a semiconductor die, likely a computer chip. You can see the intricate circuitry and the regular grid-like patterns of memory cells or processing units. The overall color scheme is in shades of sepia or brown, which is common for optical micrographs of silicon dies."
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
//     "candidatesTokenCount": 62,
//     "totalTokenCount": 328,
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
//   "responseId": "NuL7ad-jD4aIxN8Px8HzgAU"
// }
// PS C:\Users\babyface\Desktop\auwgent\Auwgent\expiremental>