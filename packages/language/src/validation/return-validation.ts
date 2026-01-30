import type { ValidationAcceptor } from 'langium';
import type { ReturnStatement } from '../generated/ast.js';
import { isInlinePromptBlock } from '../generated/ast.js';

export class ReturnValidation {
    checkReturnStatement(statement: ReturnStatement, accept: ValidationAcceptor): void {
        if (isInlinePromptBlock(statement.value)) {
            accept('error', 'Inline prompt blocks are not allowed in return statements. Use an object literal instead.', { node: statement, property: 'value' });
        }
    }
}
