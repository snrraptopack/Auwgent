declare const process: any;

export type AuwgentRuntime = any;

function isNodeLike(): boolean {
    if (typeof globalThis !== 'undefined') {
        if ((globalThis as any).navigator?.userAgent === 'Cloudflare-Workers') return false;
        if ((globalThis as any).EdgeRuntime) return false;
        if ((globalThis as any).Deno) return false;
    }

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
    try {
        return new Function('specifier', 'return import(specifier)')(specifier);
    } catch (e) {
        // Fallback for environments that disable eval/new Function (like Cloudflare Workers)
        // @ts-ignore
        return import(/* @vite-ignore */ /* webpackIgnore: true */ specifier);
    }
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

    // Use a direct, statically-analyzable dynamic import here.
    // Cloudflare Workers (Wrangler/esbuild) must be able to see this import
    // at build time so it can bundle the WASM file into the worker.
    const mod: any = await import('./wasm-runtime/auwgent_wasm_runtime.js');
    
    if (typeof mod.default === 'function') {
        await mod.default();
    }
    return new mod.AuwgentWasm(irJson);
}
