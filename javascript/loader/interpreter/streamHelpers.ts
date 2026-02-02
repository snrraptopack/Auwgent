import type { ToolArgs } from "../types/protocol";

/**
 * Async queue for streaming chunks with backpressure support.
 */
export type StreamQueue<T> = {
    push: (item: T) => void;
    close: () => void;
    fail: (err: any) => void;
    next: () => Promise<IteratorResult<T>>;
};

export const createStreamQueue = <T>(): StreamQueue<T> => {
    let closed = false;
    let error: any;
    const buffer: T[] = [];
    let pending: { resolve: (value: IteratorResult<T>) => void; reject: (error: any) => void } | null = null;

    const push = (item: T) => {
        if (closed) return;
        if (pending) {
            const { resolve } = pending;
            pending = null;
            resolve({ value: item, done: false });
            return;
        }
        buffer.push(item);
    };

    const close = () => {
        if (closed) return;
        closed = true;
        if (pending) {
            const { resolve } = pending;
            pending = null;
            resolve({ value: undefined as any, done: true });
        }
    };

    const fail = (err: any) => {
        if (closed) return;
        closed = true;
        error = err;
        if (pending) {
            const { reject } = pending;
            pending = null;
            reject(err);
        }
    };

    const next = async (): Promise<IteratorResult<T>> => {
        if (error) {
            throw error;
        }
        if (buffer.length > 0) {
            return { value: buffer.shift() as T, done: false };
        }
        if (closed) {
            return { value: undefined as any, done: true };
        }
        return new Promise<IteratorResult<T>>((resolve, reject) => {
            pending = { resolve, reject };
        });
    };

    return { push, close, fail, next };
};

/**
 * Format tool arguments for streaming display.
 */
export const formatToolArgsForStream = (args: ToolArgs | undefined): string => {
    if (!args || Object.keys(args).length === 0) {
        return "{}";
    }
    const sorted = Object.keys(args)
        .sort()
        .reduce<Record<string, any>>((acc, key) => {
            acc[key] = args[key];
            return acc;
        }, {});
    try {
        return JSON.stringify(sorted, null, 2);
    } catch (error) {
        return String(args);
    }
};
