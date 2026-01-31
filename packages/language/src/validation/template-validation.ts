import type { ValidationAcceptor } from 'langium';
import { AstUtils } from 'langium';
import type { MultilineStringLiteral, TypeConfigDeclaration, NamedPrompt, Model } from '../generated/ast.js';
import { isMultilineStringLiteral, isAgent, isHelper, isInputConfig, isContextConfig, isNamedPrompt, isFunctionCall, isPromptCall } from '../generated/ast.js';

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
                if (inputProps.length === 0) {
                    accept('error', `Input is not available in this prompt scope`, { node, property: 'value', range: this.getMatchRange(node, match.index, match[0].length) });
                    continue;
                }
                if (rest.length > 0 && !inputProps.includes(rest[0])) {
                    accept('error', `Unknown input property '${rest[0]}' in template expression`, { node, property: 'value', range: this.getMatchRange(node, match.index, match[0].length) });
                }
                continue;
            }
            if (root === 'ctx') {
                if (contextProps.length === 0) {
                    accept('error', `Context is not available in this prompt scope`, { node, property: 'value', range: this.getMatchRange(node, match.index, match[0].length) });
                    continue;
                }
                if (rest.length > 0 && !contextProps.includes(rest[0])) {
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
        const direct = this.getInputPropertiesFromContainer(node);
        if (direct.length) return direct;
        const prompt = this.getPromptContainer(node);
        if (!prompt) return [];
        const props = this.getPropertiesFromPromptUsages(prompt, 'input');
        return props.map(p => p.name);
    }

    private getContextPropertiesInScope(node: any): string[] {
        const direct = this.getContextPropertiesFromContainer(node);
        if (direct.length) return direct;
        const prompt = this.getPromptContainer(node);
        if (!prompt) return [];
        const props = this.getPropertiesFromPromptUsages(prompt, 'context');
        return props.map(p => p.name);
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

    private getInputPropertiesFromContainer(node: any): string[] {
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

    private getContextPropertiesFromContainer(node: any): string[] {
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

    private getPromptContainer(node: any): NamedPrompt | undefined {
        let current = node as any;
        while (current) {
            if (isNamedPrompt(current)) {
                return current;
            }
            current = current.$container;
        }
        return undefined;
    }

    private getPropertiesFromPromptUsages(prompt: NamedPrompt, kind: 'input' | 'context'): TypeConfigDeclaration[] {
        const document = AstUtils.getDocument(prompt);
        const root = document.parseResult?.value as Model | undefined;
        if (!root) return [];
        const collected = new Map<string, TypeConfigDeclaration>();
        for (const node of AstUtils.streamAllContents(root)) {
            if (isFunctionCall(node) && node.func?.ref === prompt) {
                const container = this.getAgentOrHelperContainer(node);
                if (container) {
                    this.collectProperties(container, kind, collected);
                }
            }
            if (isPromptCall(node) && node.prompt?.ref === prompt) {
                const container = this.getAgentOrHelperContainer(node);
                if (container) {
                    this.collectProperties(container, kind, collected);
                }
            }
        }
        return Array.from(collected.values());
    }

    private getAgentOrHelperContainer(node: any): any | undefined {
        let current = node as any;
        while (current) {
            if (isAgent(current) || isHelper(current)) {
                return current;
            }
            current = current.$container;
        }
        return undefined;
    }

    private collectProperties(container: any, kind: 'input' | 'context', collected: Map<string, TypeConfigDeclaration>): void {
        const configs = container.configs ?? [];
        for (const config of configs) {
            if (kind === 'input' && isInputConfig(config)) {
                for (const prop of config.inProperties ?? []) {
                    collected.set(prop.name, prop);
                }
            }
            if (kind === 'context' && isContextConfig(config)) {
                for (const prop of config.contextProperties ?? []) {
                    collected.set(prop.name, prop);
                }
            }
        }
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
