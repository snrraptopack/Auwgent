import type { Range } from './constraints.js';

export type TypeErrorInfo = {
    message: string;
    range?: Range;
};

export const createTypeError = (message: string, range?: Range): TypeErrorInfo => ({
    message,
    range
});
