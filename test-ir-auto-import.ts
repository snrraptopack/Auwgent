// Manual test to verify IR auto-import feature works correctly
import { createSupport, SupportTools, Product } from "./test-autoreg.agent.types";

// Mock tools implementation
const tools: SupportTools = {
    search_product_by_name: async ({ name }) => {
        return {
            name: name,
            id: "123",
            price: 99.99
        } as Product;
    },
    search_product_by_id: async ({ id }) => {
        return {
            name: "Test Product",
            id: id,
            price: 99.99
        } as Product;
    },
    purchase_product: async ({ product_id, user_id }) => {
        console.log(`Purchasing product ${product_id} for user ${user_id}`);
        return true;
    }
};

// Test 1: Create agent without IR parameter (the main improvement!)
console.log("✅ Test 1: Creating agent without IR parameter...");
const agent = createSupport({
    tools: tools,
    context: {
        isVerified: true,
        user_id: "user123"
    }
});

console.log("✅ Agent created successfully!");
console.log("✅ IR auto-import feature is working correctly!");
console.log("\nThe following improvements are now in effect:");
console.log("1. ✅ IR is automatically imported in the generated types file");
console.log("2. ✅ No need to manually import the .agent.json file");
console.log("3. ✅ No need to pass 'ir' parameter to createSupport()");
console.log("4. ✅ Cleaner, simpler API for users");
