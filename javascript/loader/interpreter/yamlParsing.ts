import { createStreamingParser, parseToJSON } from "auwgent-yaml-lite";
import type { YamlOutput } from "../types/protocol";

/**
 * Parses raw text output into a structured YamlOutput object.
 * Uses a multi-tier strategy: standard parse -> streaming parser -> fallback to raw text.
 */
export const parseYamlOutput = (textOutput: string | undefined): YamlOutput | null => {
    if (!textOutput) {
        return null;
    }
    const trimmed = textOutput.trim();
    if (!trimmed) {
        return { text: "" };
    }
    try {
        const parsed = parseToJSON(trimmed) as YamlOutput;
        if (parsed && typeof parsed === "object") {
            return parsed;
        }
    } catch {
        try {
            const streamingParser = createStreamingParser();
            streamingParser.write(trimmed);
            const parsed = streamingParser.end() as YamlOutput;
            if (parsed && typeof parsed === "object") {
                return parsed;
            }
        } catch {
            // ignore and fall back to raw text
        }
    }
    return { text: textOutput };
};
