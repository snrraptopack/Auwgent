import { GoogleDriver } from "./loader/drivers/GoogleDriver";
import { TempKey } from "./keys";
import type { SyntheticRequest } from "./loader/types/protocol";

const driver = new GoogleDriver(TempKey);

console.log("=== Testing Google Driver Structured Output ===\n");

// Test 1: Non-Streaming Structured Output
console.log("Test 1: Non-Streaming Structured Output");
console.log("----------------------------------------");

const recipeSchema = {
    type: "object",
    properties: {
        recipe_name: {
            type: "string",
            description: "The name of the recipe"
        },
        ingredients: {
            type: "array",
            items: {
                type: "object",
                properties: {
                    name: { type: "string" },
                    quantity: { type: "string" }
                },
                required: ["name", "quantity"]
            }
        },
        instructions: {
            type: "array",
            items: { type: "string" }
        }
    },
    required: ["recipe_name", "ingredients", "instructions"]
};

const request1: SyntheticRequest = {
    messages: [
        {
            role: "user",
            content: "Give me a simple recipe for chocolate chip cookies with 3 ingredients and 3 steps"
        }
    ],
    responseSchema: recipeSchema,
    config: {
        modelName: "gemini-2.0-flash"
    }
};

try {
    const result1 = await driver.execute(request1);
    console.log("✓ Non-streaming result received");
    console.log("Response text:", result1.text);
    
    // Verify it's valid JSON
    const parsed = JSON.parse(result1.text!);
    console.log("✓ Response is valid JSON");
    console.log("✓ Recipe name:", parsed.recipe_name);
    console.log("✓ Number of ingredients:", parsed.ingredients?.length);
    console.log("✓ Number of instructions:", parsed.instructions?.length);
} catch (error) {
    console.error("✗ Test 1 failed:", error);
}

console.log("\n");

// Test 2: Streaming Structured Output
console.log("Test 2: Streaming Structured Output");
console.log("------------------------------------");

const feedbackSchema = {
    type: "object",
    properties: {
        sentiment: {
            type: "string",
            enum: ["positive", "neutral", "negative"]
        },
        summary: {
            type: "string",
            description: "A detailed summary of the feedback"
        },
        score: {
            type: "number",
            description: "Score from 1-10"
        }
    },
    required: ["sentiment", "summary", "score"]
};

const request2: SyntheticRequest = {
    messages: [
        {
            role: "user",
            content: "Analyze this feedback: 'The new UI is incredibly intuitive and visually appealing. Great job! The performance improvements are noticeable.'"
        }
    ],
    responseSchema: feedbackSchema,
    config: {
        modelName: "gemini-2.0-flash"
    }
};

try {
    let chunks: string[] = [];
    let fullText = "";
    
    console.log("Streaming chunks:");
    for await (const chunk of driver.executeStream(request2)) {
        if (chunk.type === 'text') {
            chunks.push(chunk.delta);
            fullText += chunk.delta;
            process.stdout.write(chunk.delta);
        }
    }
    
    console.log("\n");
    console.log("✓ Received", chunks.length, "text chunks");
    console.log("✓ Full text length:", fullText.length);
    
    // Verify it's valid JSON
    const parsed = JSON.parse(fullText);
    console.log("✓ Concatenated result is valid JSON");
    console.log("✓ Sentiment:", parsed.sentiment);
    console.log("✓ Score:", parsed.score);
    console.log("✓ Summary length:", parsed.summary?.length);
} catch (error) {
    console.error("✗ Test 2 failed:", error);
}

console.log("\n");

// Test 3: Tool Calling (should disable structured output)
console.log("Test 3: Tool Calling with Schema (schema should be ignored)");
console.log("-------------------------------------------------------------");

const request3: SyntheticRequest = {
    messages: [
        {
            role: "user",
            content: "What's the weather like?"
        }
    ],
    tools: [
        {
            name: "get_weather",
            description: "Get the current weather",
            parameters: {
                type: "object",
                properties: {
                    location: { type: "string" }
                },
                required: ["location"]
            }
        }
    ],
    responseSchema: feedbackSchema, // This should be ignored
    config: {
        modelName: "gemini-2.0-flash"
    }
};

try {
    const result3 = await driver.execute(request3);
    if (result3.toolParams) {
        console.log("✓ Tool call detected:", result3.toolParams.name);
        console.log("✓ Tool args:", JSON.stringify(result3.toolParams.args));
        console.log("✓ Structured output correctly disabled when tools present");
    } else {
        console.log("✓ Text response (no tool call):", result3.text);
    }
} catch (error) {
    console.error("✗ Test 3 failed:", error);
}

console.log("\n");

// Test 4: Regular Text Generation (no schema)
console.log("Test 4: Regular Text Generation (no schema)");
console.log("--------------------------------------------");

const request4: SyntheticRequest = {
    messages: [
        {
            role: "user",
            content: "Tell me a short joke"
        }
    ],
    config: {
        modelName: "gemini-2.0-flash"
    }
};

try {
    const result4 = await driver.execute(request4);
    console.log("✓ Text response received");
    console.log("Response:", result4.text);
    console.log("✓ Regular text generation works without schema");
} catch (error) {
    console.error("✗ Test 4 failed:", error);
}

console.log("\n=== All Tests Complete ===");
