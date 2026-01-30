import type { Type, TypeVar } from './types.js';
import { freeTypeVars } from './types.js';

export type TypeScheme = {
    vars: string[];
    type: Type;
};

export class TypeEnv {
    private bindings: Map<string, TypeScheme>;
    private parent: TypeEnv | undefined;

    constructor(parent?: TypeEnv) {
        this.bindings = new Map();
        this.parent = parent;
    }

    set(name: string, scheme: TypeScheme): void {
        this.bindings.set(name, scheme);
    }

    get(name: string): TypeScheme | undefined {
        const local = this.bindings.get(name);
        if (local) return local;
        if (this.parent) return this.parent.get(name);
        return undefined;
    }

    extend(): TypeEnv {
        return new TypeEnv(this);
    }

    freeTypeVars(): Set<string> {
        const vars = new Set<string>();
        for (const scheme of this.bindings.values()) {
            const schemeVars = freeTypeVars(scheme.type);
            schemeVars.forEach(v => vars.add(v));
        }
        if (this.parent) {
            this.parent.freeTypeVars().forEach(v => vars.add(v));
        }
        return vars;
    }
}

export const createTypeVarGenerator = (prefix = 't') => {
    let counter = 0;
    return (): TypeVar => {
        counter += 1;
        return { kind: 'var', id: `${prefix}${counter}` };
    };
};

export const generalize = (env: TypeEnv, type: Type): TypeScheme => {
    const envVars = env.freeTypeVars();
    const typeVars = freeTypeVars(type);
    const vars = [...typeVars].filter(v => !envVars.has(v));
    return { vars, type };
};

export const instantiate = (scheme: TypeScheme, fresh: () => TypeVar): Type => {
    const substitutions = new Map<string, TypeVar>();
    const resolve = (t: Type): Type => {
        if (t.kind === 'var') {
            if (scheme.vars.includes(t.id)) {
                const existing = substitutions.get(t.id);
                if (existing) return existing;
                const next = fresh();
                substitutions.set(t.id, next);
                return next;
            }
            return t;
        }
        if (t.kind === 'func') {
            return { kind: 'func', params: t.params.map(resolve), returns: resolve(t.returns) };
        }
        if (t.kind === 'record') {
            const fields: Record<string, Type> = {};
            for (const [key, value] of Object.entries(t.fields)) {
                fields[key] = resolve(value);
            }
            return { kind: 'record', fields, optional: { ...t.optional } };
        }
        if (t.kind === 'union') {
            return { kind: 'union', options: t.options.map(resolve) };
        }
        return t;
    };
    return resolve(scheme.type);
};
