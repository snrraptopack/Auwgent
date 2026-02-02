import type { AgentIR } from "../types/ir";

export type TransferSignal = {
    __type: "TransferSignal";
    value: unknown;
    mode: "direct" | "thenContinue";
    helperName?: string;
};

/**
 * Detect if a value is a transfer signal from a workflow.
 */
export const getTransferSignal = (value: unknown): TransferSignal | null => {
    if (!value || typeof value !== "object") {
        return null;
    }
    const record = value as Record<string, unknown>;
    if (record.__type !== "TransferSignal") {
        return null;
    }
    const mode = record.mode;
    if (mode !== "direct" && mode !== "thenContinue") {
        return null;
    }
    const helperName = typeof record.helperName === "string" ? record.helperName : undefined;
    return {
        __type: "TransferSignal",
        value: record.value,
        mode,
        helperName
    };
};

/**
 * Detect if a helper result should trigger a handoff based on IR configuration.
 */
export const getHelperHandoffSignal = (
    helperName: string | undefined,
    value: unknown,
    ir: AgentIR | null
): TransferSignal | null => {
    if (!helperName || !ir?.helperHandoff) {
        return null;
    }
    const mode = ir.helperHandoff[helperName];
    if (!mode) {
        return null;
    }
    return {
        __type: "TransferSignal",
        value,
        mode: mode === "thenContinue" ? "thenContinue" : "direct",
        helperName
    };
};
