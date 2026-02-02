import type { ToolCall, YamlIntent, YamlOutput } from "../types/protocol";
import type { AgentIR } from "../types/ir";
import { logger } from "../Logger";
import { randomUUID } from "crypto";

export type IdResolver = (intent: YamlIntent, index: number) => string;

/**
 * Build tool calls from parsed YAML intents.
 * Filters out invalid intents and undeclared helpers.
 */
export const buildToolCallsFromYaml = (
    yamlOutput: YamlOutput | null,
    ir: AgentIR | null,
    idResolver?: IdResolver
): ToolCall[] => {
    if (!yamlOutput?.intents || yamlOutput.intents.length === 0) {
        return [];
    }
    const calls: ToolCall[] = [];
    yamlOutput.intents.forEach((intent, index) => {
        if (!intent || !intent.name) {
            return;
        }
        if (intent.type !== "tool_call" && intent.type !== "workflow" && intent.type !== "helper") {
            return;
        }
        if (intent.type === "helper") {
            const helperDeclared = ir?.helpers?.some(helper => helper.name === intent.name) ?? false;
            if (!helperDeclared) {
                if (intent.name === "respond") {
                    logger.debug(`[Agent] Ignoring implicit respond helper intent.`);
                    return;
                }
                logger.warn(`[Agent] Model requested unknown helper "${intent.name}" - intent skipped.`);
                return;
            }
        }
        const args = intent.args && typeof intent.args === "object" ? intent.args : {};
        const id = idResolver ? idResolver(intent, index) : randomUUID();
        calls.push({
            id,
            name: intent.name,
            args
        });
    });
    return calls;
};

/**
 * Generate a unique signature for a tool call to detect duplicates.
 */
export const getCallSignature = (call: ToolCall): string => {
    const args = call.args ?? {};
    const sortedKeys = Object.keys(args).sort();
    return `${call.name}::${JSON.stringify(args, sortedKeys)}`;
};
