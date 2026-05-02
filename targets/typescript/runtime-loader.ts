declare const process: any;

export type AuwgentRuntime = any;

function isNodeLike(): boolean {
    return (
        typeof process !== 'undefined' &&
        process?.versions != null &&
        (process.versions.node != null || process.versions.bun != null)
    );
}

function optionalRequire(): ((id: string) => any) | null {
    const metaRequire = (import.meta as any).require;
    if (typeof metaRequire === 'function') {
        return metaRequire;
    }

    const globalRequire = (globalThis as any).require;
    if (typeof globalRequire === 'function') {
        return globalRequire;
    }

    try {
        const req = (0, eval)('require');
        return typeof req === 'function' ? req : null;
    } catch {
        return null;
    }
}

function browserDynamicImport(specifier: string): Promise<any> {
    return new Function('specifier', 'return import(specifier)')(specifier);
}

export function createNativeRuntimeSync(irJson: string): AuwgentRuntime | null {
    if (!isNodeLike()) return null;

    const req = optionalRequire();
    if (!req) return null;

    const mod = req('./index.js');
    return new mod.Auwgent(irJson);
}

export async function createNativeRuntime(irJson: string): Promise<AuwgentRuntime> {
    if (isNodeLike()) {
        const nativeEntry = './index.js';
        const mod = await import(nativeEntry);
        return new mod.Auwgent(irJson);
    }

    const wasmDir = 'wasm-' + 'runtime';
    const wasmFile = 'auwgent_' + 'wasm_runtime.js';
    const mod = await browserDynamicImport(`./${wasmDir}/${wasmFile}`);
    if (typeof mod.default === 'function') {
        await mod.default();
    }
    return new mod.AuwgentWasm(irJson);
}
