import type { ContentBlock, DriverResult, YamlOutput } from "../types/protocol";
import type { AgentIR } from "../types/ir";
import { formatAssistantDisplay } from "./assistantFormatting";

/**
 * Extract plain text from a driver result's content blocks.
 */
export const extractText = (result: DriverResult): string | undefined => {
    if (!result.content || result.content.length === 0) {
        return undefined;
    }
    return result.content
        .filter(block => block.type === "text")
        .map(block => block.type === "text" ? block.text : "")
        .join("");
};

/**
 * Resolve the final output value from YAML or raw text based on IR output schema.
 */
export const resolveFinalOutput = <TOutput>(
    yamlOutput: YamlOutput | null,
    textOutput: string | undefined,
    ir: AgentIR | null
): TOutput => {
    const hasOutput = !!ir?.output && Object.keys(ir.output).length > 0;
    if (hasOutput) {
        if (!yamlOutput || typeof yamlOutput !== "object") {
            throw new Error("Model failed to return valid YAML");
        }
        const output = yamlOutput.output ?? yamlOutput;
        if (!output || typeof output !== "object") {
            throw new Error("Model returned malformed YAML output block");
        }
        return output as TOutput;
    }
    if (yamlOutput) {
        if (yamlOutput.output && typeof yamlOutput.output === "object") {
            return yamlOutput.output as unknown as TOutput;
        }
        if (yamlOutput.text !== undefined) {
            return String(yamlOutput.text ?? "") as TOutput;
        }
    }
    return (textOutput ?? "") as TOutput;
};

/**
 * Build assistant content blocks for message history.
 */
export const buildAssistantContent = (
    result: DriverResult,
    yaml: YamlOutput | null,
    rawText: string | undefined
): ContentBlock[] => {
    const textBlocks = (text: string): ContentBlock[] => {
        if (!text) {
            return [];
        }
        return [{ type: "text", text }];
    };

    if (result.content && result.content.length > 0) {
        const hasNonText = result.content.some(block => block.type !== "text");
        if (hasNonText) {
            return result.content;
        }
        const combined = result.content
            .filter(block => block.type === "text")
            .map(block => (block.type === "text" ? block.text : ""))
            .join("");
        const formattedFromContent = formatAssistantDisplay(yaml, combined);
        if (formattedFromContent) {
            return textBlocks(formattedFromContent);
        }
        if (combined.trim().length > 0) {
            return textBlocks(combined);
        }
    }

    const formatted = formatAssistantDisplay(yaml, rawText);
    if (formatted) {
        return textBlocks(formatted);
    }

    return textBlocks(rawText ?? "");
};
