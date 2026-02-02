import type { YamlIntent, YamlOutput } from "../types/protocol";

export type AssistantSummary = {
    text?: string;
    question?: string;
    output?: Record<string, any>;
    intents?: YamlIntent[];
};

const extractTextFromRawYaml = (raw: string | undefined): string | null => {
    if (!raw) {
        return null;
    }
    const lines = raw.split(/\r?\n/);
    for (const line of lines) {
        const trimmed = line.trim();
        if (!trimmed.toLowerCase().startsWith("text:")) {
            continue;
        }
        let value = trimmed.slice(5).trim();
        if (value.length === 0) {
            continue;
        }
        if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
            value = value.slice(1, -1);
        }
        return value.length > 0 ? value : null;
    }
    return null;
};

const extractQuestionFromRawYaml = (raw: string | undefined): string | null => {
    if (!raw) {
        return null;
    }
    const lines = raw.split(/\r?\n/);
    for (const line of lines) {
        const trimmed = line.trim();
        if (!trimmed.toLowerCase().startsWith("question:")) {
            continue;
        }
        let value = trimmed.slice(9).trim();
        if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
            value = value.slice(1, -1);
        }
        if (value.toLowerCase() === "null") {
            return null;
        }
        return value.length > 0 ? value : null;
    }
    return null;
};

export const buildAssistantSummary = (yaml: YamlOutput | null, rawText: string | undefined): AssistantSummary => {
    const summary: AssistantSummary = {};

    if (yaml && typeof yaml === "object") {
        if (typeof yaml.text === "string" && yaml.text.trim().length > 0) {
            summary.text = yaml.text.trim();
        }
        if (yaml.output !== undefined) {
            summary.output = yaml.output;
        }
        if (typeof yaml.question === "string" && yaml.question.trim().length > 0 && yaml.question.trim().toLowerCase() !== "null") {
            summary.question = yaml.question.trim();
        }
        const actionableIntents = yaml.intents?.filter(intent => {
            if (!intent || typeof intent !== "object") {
                return false;
            }
            const normalizedType = typeof intent.type === "string" ? intent.type.trim().toLowerCase() : "";
            const normalizedName = typeof intent.name === "string" ? intent.name.trim().toLowerCase() : "";
            // Filter out "respond" intents - either by type or by name
            if (normalizedType === "respond" || normalizedName === "respond") {
                return false;
            }
            return normalizedType.length > 0;
        });
        if (actionableIntents && actionableIntents.length > 0) {
            summary.intents = actionableIntents;
        }
        if (summary.text || summary.question || summary.output || (summary.intents && summary.intents.length > 0)) {
            return summary;
        }
    }

    const textFromRaw = extractTextFromRawYaml(rawText);
    if (textFromRaw) {
        summary.text = textFromRaw;
    }
    const questionFromRaw = extractQuestionFromRawYaml(rawText);
    if (questionFromRaw) {
        summary.question = questionFromRaw;
    }
    return summary;
};

export const formatAssistantDisplay = (yaml: YamlOutput | null, rawText: string | undefined): string | null => {
    const summary = buildAssistantSummary(yaml, rawText);
    const segments: string[] = [];
    if (summary.text) {
        segments.push(summary.text);
    }
    if (summary.output) {
        segments.push(`Output:\n${JSON.stringify(summary.output, null, 2)}`);
    }
    if (summary.question) {
        segments.push(`Question: ${summary.question}`);
    }
    if (summary.intents && summary.intents.length > 0) {
        segments.push(`Intents:\n${JSON.stringify(summary.intents, null, 2)}`);
    }
    if (segments.length === 0) {
        return null;
    }
    return segments.join("\n\n");
};

export const formatStreamingPreview = (yaml: YamlOutput | null): { display: string | null; raw?: string } => {
    const summary = buildAssistantSummary(yaml, undefined);
    const hasIntents = Array.isArray(summary.intents) && summary.intents.length > 0;

    // Build preview from what the model ACTUALLY returned - no artificial fields
    const preview: Record<string, any> = {};

    // Always include text if present
    if (summary.text) {
        preview.text = summary.text;
    }

    // Include output if present
    if (summary.output !== undefined) {
        preview.output = summary.output;
    }

    // Include question if present
    if (summary.question) {
        preview.question = summary.question;
    }

    // Include intents if there are actionable ones
    if (hasIntents) {
        preview.intents = summary.intents;
    }

    if (Object.keys(preview).length === 0) {
        return { display: null };
    }

    return {
        display: JSON.stringify(preview, null, 2),
        raw: summary.text || undefined
    };
};
