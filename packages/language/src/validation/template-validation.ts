import type { ValidationAcceptor } from 'langium';
import { AstUtils } from 'langium';
import type { MultilineStringLiteral } from '../generated/ast.js';
import { isMultilineStringLiteral, isAgent, isHelper, isInputConfig, isContextConfig, isNamedPrompt } from '../generated/ast.js';

export class TemplateValidation {
    checkTemplateInterpolations(node: MultilineStringLiteral, accept: ValidationAcceptor): void {
        if (!isMultilineStringLiteral(node)) return;
        const raw = node.value ?? '';
        const content = raw.replace(/^"""/, '').replace(/"""$/, '');
        const interpolationPattern = /\{\{([^}]+)\}\}/g;
        const inputProps = this.getInputPropertiesInScope(node);
        const contextProps = this.getContextPropertiesInScope(node);
        const promptParams = this.getPromptParamsInScope(node);
        const hasKnownScope = inputProps.length > 0 || contextProps.length > 0 || promptParams.length > 0;

        let match: RegExpExecArray | null;
        while ((match = interpolationPattern.exec(content)) !== null) {
            const expr = match[1].trim();
            if (!this.isSimpleTemplateExpression(expr)) {
                accept('error', `Invalid template expression: ${expr}`, { node, property: 'value', range: this.getMatchRange(node, match.index, match[0].length) });
                continue;
            }
            const [root, ...rest] = expr.split('.');
            if (!root) continue;
            if (root === 'input') {
                if (rest.length > 0 && inputProps.length > 0 && !inputProps.includes(rest[0])) {
                    accept('error', `Unknown input property '${rest[0]}' in template expression`, { node, property: 'value', range: this.getMatchRange(node, match.index, match[0].length) });
                }
                continue;
            }
            if (root === 'ctx') {
                if (rest.length > 0 && contextProps.length > 0 && !contextProps.includes(rest[0])) {
                    accept('error', `Unknown context property '${rest[0]}' in template expression`, { node, property: 'value', range: this.getMatchRange(node, match.index, match[0].length) });
                }
                continue;
            }
            if (hasKnownScope && !inputProps.includes(root) && !contextProps.includes(root) && !promptParams.includes(root)) {
                accept('error', `Unknown template reference '${root}'`, { node, property: 'value', range: this.getMatchRange(node, match.index, match[0].length) });
            }
        }
    }

    private isSimpleTemplateExpression(expr: string): boolean {
        return /^[_a-zA-Z][\w_]*(\.[_a-zA-Z][\w_]*)*$/.test(expr);
    }

    private getInputPropertiesInScope(node: any): string[] {
        let current = node as any;
        while (current) {
            if (isAgent(current) || isHelper(current)) {
                const configs = current.configs ?? [];
                for (const config of configs) {
                    if (isInputConfig(config)) {
                        return (config.inProperties ?? []).map((p: any) => p.name);
                    }
                }
            }
            current = current.$container;
        }
        return [];
    }

    private getContextPropertiesInScope(node: any): string[] {
        let current = node as any;
        while (current) {
            if (isAgent(current) || isHelper(current)) {
                const configs = current.configs ?? [];
                for (const config of configs) {
                    if (isContextConfig(config)) {
                        return (config.contextProperties ?? []).map((p: any) => p.name);
                    }
                }
            }
            current = current.$container;
        }
        return [];
    }

    private getPromptParamsInScope(node: any): string[] {
        let current = node as any;
        while (current) {
            if (isNamedPrompt(current)) {
                return (current as any).params?.map((p: any) => p.name) ?? [];
            }
            current = current.$container;
        }
        return [];
    }

    private getMatchRange(node: MultilineStringLiteral, matchIndex: number, matchLength: number) {
        const cstNode = node.$cstNode;
        if (!cstNode) return undefined;
        const document = AstUtils.getDocument(node);
        const raw = node.value ?? '';
        const prefixLength = raw.startsWith('"""') ? 3 : 0;
        const baseOffset = cstNode.offset + prefixLength;
        const startOffset = baseOffset + matchIndex;
        const endOffset = startOffset + matchLength;
        return {
            start: document.textDocument.positionAt(startOffset),
            end: document.textDocument.positionAt(endOffset)
        };
    }
}
