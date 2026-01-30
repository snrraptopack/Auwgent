import type { Constraint } from './constraints.js';
import type { Type } from './types.js';
import { tError } from './types.js';
import { TypeEnv, createTypeVarGenerator } from './env.js';

export type InferenceResult = {
    type: Type;
    constraints: Constraint[];
};

export class TypeInferencer {
    private env: TypeEnv;
    private fresh: () => Type;

    constructor(env?: TypeEnv) {
        this.env = env ?? new TypeEnv();
        this.fresh = createTypeVarGenerator();
    }

    inferExpression(node: unknown): InferenceResult {
        void node;
        const env = this.env;
        void env;
        const type = this.fresh();
        return { type, constraints: [] };
    }

    inferModel(node: unknown): InferenceResult {
        void node;
        const env = this.env;
        void env;
        return { type: tError('not-inferred'), constraints: [] };
    }
}
