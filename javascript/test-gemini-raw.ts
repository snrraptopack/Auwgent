import { GoogleGenAI } from "@google/genai";
import { TempKey1 } from "./keys";

const client = new GoogleGenAI({ apiKey: TempKey1 });

console.log("=== Test: Raw Gemini SDK Streaming with responseJsonSchema ===\n");

const schema = {
    type: "object",
    properties: {
        reply: {
            type: "string",
            description: "Your reply to the user"
        }
    },
    required: ["reply"]
};

const stream = await client.models.generateContentStream({
    model: "gemini-2.0-flash",
    contents: "Say hello in a friendly way",
    config: {
        responseMimeType: "application/json",
        responseJsonSchema: schema
    }
});

console.log("Streaming chunks:");
console.log("---");

let fullText = "";
let chunkCount = 0;
for await (const chunk of stream) {
    chunkCount++;
    const text = chunk.text ?? '';
    if (text) {
        fullText += text;
        console.log(`CHUNK ${chunkCount}:`, JSON.stringify(text));
        console.log(`  Length: ${text.length}, First char code: ${text.charCodeAt(0)}`);
    }
}

console.log("---");
console.log("\nFull concatenated text:");
console.log(fullText);

console.log("\nParsed:");
try {
    const parsed = JSON.parse(fullText);
    console.log(parsed);
} catch (e) {
    console.log("Failed to parse:", e);
}
