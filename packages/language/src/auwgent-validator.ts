import type { ValidationAcceptor, ValidationChecks } from 'langium';
import type { AuwgentAstType, ReturnStatement } from './generated/ast.js';
import { isInlinePromptBlock } from './generated/ast.js';
import type { AuwgentServices } from './auwgent-module.js';

/**
 * Register custom validation checks.
 */
export function registerValidationChecks(services: AuwgentServices) {
    const registry = services.validation.ValidationRegistry;
    const validator = services.validation.AuwgentValidator;
    const checks: ValidationChecks<AuwgentAstType> = {
        ReturnStatement: validator.checkReturnStatement
    };
    registry.register(checks, validator);
}

/**
 * Implementation of custom validations.
 */
export class AuwgentValidator {
    checkReturnStatement(statement: ReturnStatement, accept: ValidationAcceptor): void {
        if (isInlinePromptBlock(statement.value)) {
            accept('error', 'Inline prompt blocks are not allowed in return statements. Use an object literal instead.', { node: statement, property: 'value' });
        }
    }
}
