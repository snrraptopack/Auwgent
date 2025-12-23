import type { AgentDriver } from "./types/protocol";

// Built-in provider patterns for common model naming conventions
const PROVIDER_PATTERNS = {
    google: /^gemini/i,
    openai: /^(gpt-|o1-|o3-|chatgpt)/i,
    anthropic: /^claude/i,
    meta: /^llama/i,
    mistral: /^(mistral|mixtral)/i,
    deepseek: /^deepseek/i,
    // OpenAI-compatible providers (use same driver as openai with different base URL)
    kimi: /^(kimi|moonshot)/i,
    together: /^together/i,
    groq: /^(groq|llama.*groq)/i,
} as const;

export type KnownProvider = keyof typeof PROVIDER_PATTERNS;

export class DriverRegistry {
    private models = new Map<string, AgentDriver>();
    private patterns = new Map<RegExp, AgentDriver>();
    private providers = new Map<string, AgentDriver>();

    /**
     * Register a driver for a specific model name (Exact Match)
     * e.g. registry.registerModel("moonshot-v1", myCustomDriver);
     */
    registerModel(model: string, driver: AgentDriver) {
        this.models.set(model, driver);
    }

    /**
     * Register a driver for a pattern (Pattern Match)
     * e.g. registry.registerPattern(/^gpt-/, openAiDriver);
     */
    registerPattern(pattern: RegExp, driver: AgentDriver) {
        this.patterns.set(pattern, driver);
    }

    /**
     * Register a driver for a known provider (Provider Match)
     * Uses built-in patterns for common providers.
     * e.g. registry.registerProvider("google", googleDriver);
     *      registry.registerProvider("openai", openAiDriver);
     */
    registerProvider(provider: KnownProvider, driver: AgentDriver) {
        this.providers.set(provider, driver);
    }

    /**
     * Find the correct driver for a model name
     * Resolution order: Exact Model -> Custom Pattern -> Provider Pattern
     */
    resolve(modelName: string): AgentDriver | undefined {
        // Priority 1: Exact Match
        if (this.models.has(modelName)) {
            return this.models.get(modelName);
        }

        // Priority 2: Custom Pattern Match
        for (const [pattern, driver] of this.patterns) {
            if (pattern.test(modelName)) {
                return driver;
            }
        }

        // Priority 3: Provider Pattern Match
        for (const [providerName, driver] of this.providers) {
            const pattern = PROVIDER_PATTERNS[providerName as KnownProvider];
            if (pattern && pattern.test(modelName)) {
                return driver;
            }
        }

        return undefined;
    }
}