import type { Type } from './types.js';
import { isTypeVar, freeTypeVars } from './types.js';

export type Substitution = Record<string, Type>;

export class UnificationError extends Error {
    left: Type;
    right: Type;

    constructor(message: string, left: Type, right: Type) {
        super(message);
        this.left = left;
        this.right = right;
    }
}

export const applySubstitution = (type: Type, subst: Substitution): Type => {
    if (type.kind === 'var') {
        const replacement = subst[type.id];
        if (replacement) {
            return applySubstitution(replacement, subst);
        }
        return type;
    }
    if (type.kind === 'func') {
        return {
            kind: 'func',
            params: type.params.map(t => applySubstitution(t, subst)),
            returns: applySubstitution(type.returns, subst)
        };
    }
    if (type.kind === 'array') {
        return { kind: 'array', element: applySubstitution(type.element, subst) };
    }
    if (type.kind === 'record') {
        const fields: Record<string, Type> = {};
        for (const [key, value] of Object.entries(type.fields)) {
            fields[key] = applySubstitution(value, subst);
        }
        return { kind: 'record', fields, optional: { ...type.optional } };
    }
    if (type.kind === 'union') {
        return { kind: 'union', options: type.options.map(t => applySubstitution(t, subst)) };
    }
    return type;
};

const occursIn = (id: string, type: Type, subst: Substitution): boolean => {
    const resolved = applySubstitution(type, subst);
    if (resolved.kind === 'var') {
        return resolved.id === id;
    }
    return freeTypeVars(resolved).has(id);
};

export const unifyTypes = (left: Type, right: Type, subst: Substitution = {}): Substitution => {
    const l = applySubstitution(left, subst);
    const r = applySubstitution(right, subst);

    if (isTypeVar(l)) {
        if (r.kind === 'var' && l.id === r.id) {
            return subst;
        }
        if (occursIn(l.id, r, subst)) {
            throw new UnificationError(`Recursive type for ${l.id}`, l, r);
        }
        return { ...subst, [l.id]: r };
    }
    if (isTypeVar(r)) {
        return unifyTypes(r, l, subst);
    }
    if (l.kind === 'const' && r.kind === 'const') {
        if (l.name !== r.name) {
            throw new UnificationError(`Type mismatch: ${l.name} vs ${r.name}`, l, r);
        }
        return subst;
    }
    if (l.kind === 'func' && r.kind === 'func') {
        if (l.params.length !== r.params.length) {
            throw new UnificationError(`Arity mismatch: ${l.params.length} vs ${r.params.length}`, l, r);
        }
        let s = subst;
        for (let i = 0; i < l.params.length; i += 1) {
            s = unifyTypes(l.params[i], r.params[i], s);
        }
        return unifyTypes(l.returns, r.returns, s);
    }
    if (l.kind === 'array' && r.kind === 'array') {
        return unifyTypes(l.element, r.element, subst);
    }
    if (l.kind === 'record' && r.kind === 'record') {
        let s = subst;
        for (const [key, value] of Object.entries(l.fields)) {
            const other = r.fields[key];
            if (!other) {
                throw new UnificationError(`Missing field '${key}'`, l, r);
            }
            s = unifyTypes(value, other, s);
        }
        return s;
    }
    if (l.kind === 'union' && r.kind === 'union') {
        if (l.options.length !== r.options.length) {
            throw new UnificationError(`Union size mismatch`, l, r);
        }
        let s = subst;
        for (let i = 0; i < l.options.length; i += 1) {
            s = unifyTypes(l.options[i], r.options[i], s);
        }
        return s;
    }
    throw new UnificationError(`Type mismatch`, l, r);
};
