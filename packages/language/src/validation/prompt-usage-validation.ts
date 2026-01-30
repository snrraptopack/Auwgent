import type { ValidationAcceptor } from 'langium';
import type { PromptCall, VariableRef, FunctionCall } from '../generated/ast.js';
import { isNamedPrompt, isVariableRef, isPromptCall, isFunctionCall } from '../generated/ast.js';

export class PromptUsageValidation {
    checkPromptRefUsage(node: VariableRef, accept: ValidationAcceptor): void {
        if (!isVariableRef(node)) return;
        const ref = node.variable.ref;
        if (ref && isNamedPrompt(ref) && (ref.params?.length ?? 0) > 0) {
            accept('error', `Prompt '${ref.name}' requires arguments. Use prompt ${ref.name}(...)`, { node, property: 'variable' });
        }
    }

    checkPromptCall(node: PromptCall, accept: ValidationAcceptor): void {
        if (!isPromptCall(node)) return;
        const ref = node.prompt.ref;
        if (!ref || !isNamedPrompt(ref)) return;
        const expected = ref.params?.length ?? 0;
        const actual = node.args?.length ?? 0;
        if (expected !== actual) {
            accept('error', `Prompt '${ref.name}' expects ${expected} argument(s) but got ${actual}`, { node, property: 'args' });
        }
    }

    checkFunctionCall(node: FunctionCall, accept: ValidationAcceptor): void {
        if (!isFunctionCall(node)) return;
        const ref = node.func.ref;
        if (!ref || !isNamedPrompt(ref)) return;
        const expected = ref.params?.length ?? 0;
        const actual = node.args?.length ?? 0;
        if (expected !== actual) {
            accept('error', `Prompt '${ref.name}' expects ${expected} argument(s) but got ${actual}`, { node, property: 'args' });
        }
    }
}
