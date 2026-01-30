export type TypeId = string;

export type Type =
    | TypeVar
    | TypeConst
    | TypeFunc
    | TypeArray
    | TypeRecord
    | TypeUnion
    | TypeError;

export type TypeVar = {
    kind: 'var';
    id: TypeId;
};

export type TypeConst = {
    kind: 'const';
    name: string;
};

export type TypeFunc = {
    kind: 'func';
    params: Type[];
    returns: Type;
};

export type TypeArray = {
    kind: 'array';
    element: Type;
};

export type TypeRecord = {
    kind: 'record';
    fields: Record<string, Type>;
    optional: Record<string, boolean>;
};

export type TypeUnion = {
    kind: 'union';
    options: Type[];
};

export type TypeError = {
    kind: 'error';
    message: string;
};

export const tVar = (id: TypeId): TypeVar => ({ kind: 'var', id });
export const tConst = (name: string): TypeConst => ({ kind: 'const', name });
export const tFunc = (params: Type[], returns: Type): TypeFunc => ({ kind: 'func', params, returns });
export const tArray = (element: Type): TypeArray => ({ kind: 'array', element });
export const tRecord = (fields: Record<string, Type>, optional: Record<string, boolean>): TypeRecord => ({ kind: 'record', fields, optional });
export const tUnion = (options: Type[]): TypeUnion => ({ kind: 'union', options });
export const tError = (message: string): TypeError => ({ kind: 'error', message });

export const isTypeVar = (type: Type): type is TypeVar => type.kind === 'var';

export const freeTypeVars = (type: Type): Set<string> => {
    const vars = new Set<string>();
    const visit = (t: Type) => {
        if (t.kind === 'var') {
            vars.add(t.id);
            return;
        }
        if (t.kind === 'func') {
            t.params.forEach(visit);
            visit(t.returns);
            return;
        }
        if (t.kind === 'array') {
            visit(t.element);
            return;
        }
        if (t.kind === 'record') {
            Object.values(t.fields).forEach(visit);
            return;
        }
        if (t.kind === 'union') {
            t.options.forEach(visit);
        }
    };
    visit(type);
    return vars;
};
