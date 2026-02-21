import type { ValidationAcceptor } from 'langium';
import { AstUtils } from 'langium';
import type { MultilineStringLiteral, TypeConfigDeclaration, NamedPrompt, Model } from '../generated/ast.js';
import { isMultilineStringLiteral, isAgent, isHelper, isInputConfig, isContextConfig, isNamedPrompt, isFunctionCall, isPromptCall } from '../generated/ast.js';
import { TypeChecker } from '../type-system/checker.js';

const VALID_OPERATORS = ['==', '!=', '>=', '<=', '>', '<'];
const VALID_SCHEMA_ROOTS = ['output', 'input', 'context', 'types'];

export class TemplateValidation {
    checkTemplateInterpolations(node: MultilineStringLiteral, accept: ValidationAcceptor): void {
        if (!isMultilineStringLiteral(node)) return;
        const raw = node.value ?? '';
        const content = raw.replace(/^"""/, '').replace(/"""$/, '');

        // Check for unclosed {{#if}} blocks
        this.checkIfBlockBalance(node, content, accept);

        // Check for invalid conditionals (missing operators)
        this.checkConditionOperators(node, content, accept);

        // Check @schema directives
        this.checkSchemaDirectives(node, content, accept);

        // Existing variable interpolation checks
        // Exclude #, @, / to avoid matching {{#if}}, {{@schema}}, {{/if}}, {{else}}
        const interpolationPattern = /\{\{([^}#@/]+)\}\}/g;
        const inputProps = this.getInputPropertiesInScope(node);
        const contextProps = this.getContextPropertiesInScope(node);
        const promptParams = this.getPromptParamsInScope(node);
        const hasKnownScope = inputProps.length > 0 || contextProps.length > 0 || promptParams.length > 0;

        let match: RegExpExecArray | null;
        while ((match = interpolationPattern.exec(content)) !== null) {
            let expr = match[1].trim();

            // Skip {{else}} - it's part of if/else control flow
            if (expr === 'else') continue;

            if (!this.isSimpleTemplateExpression(expr)) {
                accept('error', `Invalid template expression: ${expr}`, { node, property: 'value', range: this.getMatchRange(node, match.index, match[0].length) });
                continue;
            }

            const [root, ...rest] = expr.split('.');
            if (!root) continue;
            // Semantic type checking for property access
            const document = AstUtils.getDocument(node);
            const model = document.parseResult?.value as Model | undefined;
            if (model) {
                const checker = new TypeChecker(model);
                const env = checker.buildEnvForNode(node);

                // Root must exist in scope
                let rootScope = '';
                if (root === 'input' || root === 'ctx' || inputProps.includes(root) || contextProps.includes(root) || promptParams.includes(root)) {
                    rootScope = root;
                } else {
                    if (hasKnownScope) {
                        accept('error', `Unknown template reference '${root}'`, { node, property: 'value', range: this.getMatchRange(node, match.index, match[0].length) });
                    }
                    continue;
                }

                if (rest.length > 0) {
                    // Check literal 'input'/'ctx' prefix keywords first since they aren't normal variables in the Env
                    if (rootScope === 'input') {
                        if (!inputProps.includes(rest[0])) {
                            accept('error', `Unknown input property '${rest[0]}' in template expression`, { node, property: 'value', range: this.getMatchRange(node, match.index, match[0].length) });
                            continue;
                        }
                        // To check further than `input.x.y`, we can check `x.y` from env
                        expr = rest.join('.');
                    } else if (rootScope === 'ctx') {
                        if (!contextProps.includes(rest[0])) {
                            accept('error', `Unknown context property '${rest[0]}' in template expression`, { node, property: 'value', range: this.getMatchRange(node, match.index, match[0].length) });
                            continue;
                        }
                        // To check further than `ctx.x.y`, we can check `x.y` from env
                        expr = rest.join('.');
                    }

                    const path = expr.split('.');
                    const typeResult = checker.inferPathType(path, env);

                    if (typeResult.kind === 'error') {
                        accept('error', typeResult.message, { node, property: 'value', range: this.getMatchRange(node, match.index, match[0].length) });
                    }
                }
            }
        }
    }

    /**
     * Check that all {{#if}} blocks are properly closed with {{/if}}
     */
    private checkIfBlockBalance(node: MultilineStringLiteral, content: string, accept: ValidationAcceptor): void {
        // Match {{#if with optional whitespace - catches both {{#if}} and {{#if condition}}
        const ifOpenPattern = /\{\{#if(\s*)/g;
        const ifClosePattern = /\{\{\/if\}\}/g;

        const opens: { index: number, length: number }[] = [];
        const closes: number[] = [];

        let match: RegExpExecArray | null;

        // Find all {{#if ...}}
        while ((match = ifOpenPattern.exec(content)) !== null) {
            // Find the closing }}
            const closeIndex = content.indexOf('}}', match.index);
            if (closeIndex === -1) {
                accept('error', `Unclosed {{#if block - missing }}`, {
                    node, property: 'value',
                    range: this.getMatchRange(node, match.index, match[0].length)
                });
            } else {
                // Check if there's a condition between {{#if and }}
                const fullMatch = content.slice(match.index, closeIndex + 2);
                const conditionPart = fullMatch.slice(5, -2).trim(); // Remove {{#if and }}

                if (!conditionPart) {
                    accept('error', `{{#if}} requires a condition, e.g., {{#if value == "expected"}}`, {
                        node, property: 'value',
                        range: this.getMatchRange(node, match.index, fullMatch.length)
                    });
                }

                opens.push({ index: match.index, length: closeIndex - match.index + 2 });
            }
        }

        // Find all {{/if}}
        while ((match = ifClosePattern.exec(content)) !== null) {
            closes.push(match.index);
        }

        // Check balance
        if (opens.length > closes.length) {
            // Report error on each unclosed {{#if}}
            for (let i = closes.length; i < opens.length; i++) {
                const open = opens[i];
                accept('error', `Unclosed {{#if}} block - missing {{/if}}`, {
                    node, property: 'value',
                    range: this.getMatchRange(node, open.index, open.length)
                });
            }
        } else if (closes.length > opens.length) {
            accept('error', `Found {{/if}} without matching {{#if}}`, {
                node, property: 'value'
            });
        }

        // Check for {{else}} without {{#if}}
        const elsePattern = /\{\{else\}\}/g;
        const elseMatches: number[] = [];
        while ((match = elsePattern.exec(content)) !== null) {
            elseMatches.push(match.index);
        }

        // Each {{else}} should be between an {{#if}} and {{/if}}
        for (const elseIndex of elseMatches) {
            const hasOpenBefore = opens.some(o => o.index < elseIndex);
            const hasCloseAfter = closes.some(c => c > elseIndex);
            if (!hasOpenBefore || !hasCloseAfter) {
                accept('error', `{{else}} must be inside an {{#if}}...{{/if}} block`, {
                    node, property: 'value',
                    range: this.getMatchRange(node, elseIndex, 8)
                });
            }
        }
    }

    /**
     * Check that {{#if condition}} has a valid comparison operator
     */
    private checkConditionOperators(node: MultilineStringLiteral, content: string, accept: ValidationAcceptor): void {
        // Match {{#if condition}}
        const ifPattern = /\{\{#if\s+(.+?)\}\}/g;

        let match: RegExpExecArray | null;
        while ((match = ifPattern.exec(content)) !== null) {
            const condition = match[1].trim();

            // Check if condition has a valid operator
            const hasValidOperator = VALID_OPERATORS.some(op => condition.includes(op));

            if (!hasValidOperator) {
                accept('error', `Condition "${condition}" requires an explicit comparison operator (==, !=, >, <, >=, <=). Truthy checks are not supported for cross-language compatibility.`, {
                    node, property: 'value',
                    range: this.getMatchRange(node, match.index, match[0].length)
                });
            }
        }
    }

    /**
     * Check {{@schema(path)}} directives for valid paths
     */
    private checkSchemaDirectives(node: MultilineStringLiteral, content: string, accept: ValidationAcceptor): void {
        const schemaPattern = /\{\{@schema\(([^)]*)\)\}\}/g;

        let match: RegExpExecArray | null;
        while ((match = schemaPattern.exec(content)) !== null) {
            const path = match[1].trim();

            if (!path) {
                accept('error', `{{@schema()}} requires a path argument`, {
                    node, property: 'value',
                    range: this.getMatchRange(node, match.index, match[0].length)
                });
                continue;
            }

            const parts = path.split('.');
            const root = parts[0];

            if (!VALID_SCHEMA_ROOTS.includes(root)) {
                accept('error', `Invalid @schema path "${root}". Valid roots: ${VALID_SCHEMA_ROOTS.join(', ')}`, {
                    node, property: 'value',
                    range: this.getMatchRange(node, match.index, match[0].length)
                });
                continue;
            }

            // Check types.TypeName requires a type name
            if (root === 'types' && parts.length < 2) {
                accept('error', `{{@schema(types.TypeName)}} requires a type name`, {
                    node, property: 'value',
                    range: this.getMatchRange(node, match.index, match[0].length)
                });
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
