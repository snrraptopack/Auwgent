import type { Type } from './types.js';

export type Position = {
    line: number;
    character: number;
};

export type Range = {
    start: Position;
    end: Position;
};

export type Constraint = {
    left: Type;
    right: Type;
    range?: Range;
    message?: string;
};
