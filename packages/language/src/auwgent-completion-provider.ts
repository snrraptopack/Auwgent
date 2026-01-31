import { CompletionAcceptor, CompletionContext, CompletionValueItem, DefaultCompletionProvider, NextFeature, LangiumServices, AstNodeHoverProvider } from 'langium/lsp';
import { AstNode, AstUtils } from 'langium';
import type { Hover } from 'vscode-languageserver';
import type { BaseType, Model, MultilineStringLiteral, NamedPrompt, TypeConfigDeclaration, TypeDeclaration, Types } from './generated/ast.js';
import { isFileImport, isNamedImports, isAgent, isHelper, isInputConfig, isContextConfig, isNamedPrompt, isFunctionCall, isPromptCall, isMultilineStringLiteral, isArrayType, isBaseType, isObjectType, isUnionType } from './generated/ast.js';

/**
 * Custom completion provider for Auwgent
 * Provides autocomplete for import paths and imported symbols
 */
export class AuwgentCompletionProvider extends DefaultCompletionProvider {
    private readonly documents: LangiumServices['shared']['workspace']['LangiumDocuments'];
    private readonly workspaceLock: LangiumServices['shared']['workspace']['WorkspaceLock'];
    
    constructor(services: LangiumServices) {
        super(services);
        this.documents = services.shared.workspace.LangiumDocuments;
        this.workspaceLock = services.shared.workspace.WorkspaceLock;
    }
    
    override completionFor(context: CompletionContext, next: NextFeature, acceptor: CompletionAcceptor): void {
        if (this.isTemplateInterpolationContext(context)) {
            this.completeTemplateInterpolation(context, acceptor);
            return;
        }

        // Check if we're completing an import path
        if (this.isImportPathContext(context)) {
            this.completeImportPath(context, acceptor);
            return;
        }
        
        // Check if we're completing imported symbols
        if (this.isImportSymbolContext(context)) {
            this.completeImportedSymbols(context, acceptor);
            return;
        }
        
        // Fall back to default completion
        super.completionFor(context, next, acceptor);
    }

    private isTemplateInterpolationContext(context: CompletionContext): boolean {
        const textDocument = context.document?.textDocument;
        if (!textDocument) return false;
        const offset = textDocument.offsetAt(context.position);
        const text = textDocument.getText();
        const openIndex = text.lastIndexOf('{{', offset);
        if (openIndex === -1) return false;
        const closeIndex = text.lastIndexOf('}}', offset);
        return openIndex > closeIndex;
    }

    private completeTemplateInterpolation(context: CompletionContext, acceptor: CompletionAcceptor): void {
        const textDocument = context.document?.textDocument;
        if (!textDocument) return;
        const offset = textDocument.offsetAt(context.position);
        const text = textDocument.getText();
        const openIndex = text.lastIndexOf('{{', offset);
        if (openIndex === -1) return;

        const current = text.slice(openIndex + 2, offset);
        const token = current.trim();
        const dotIndex = token.indexOf('.');
        const rootToken = dotIndex === -1 ? token : token.slice(0, dotIndex);

        const inputProps = getInputPropertiesInScope(context.node);
        const contextProps = getContextPropertiesInScope(context.node);
        const promptParams = getPromptParamsInScope(context.node);

        if (dotIndex !== -1) {
            if (rootToken === 'input') {
                this.acceptList(context, acceptor, inputProps, 'Input property');
                return;
            }
            if (rootToken === 'ctx') {
                this.acceptList(context, acceptor, contextProps, 'Context property');
                return;
            }
        }

        this.acceptList(context, acceptor, ['input', 'ctx'], 'Template root');
        this.acceptList(context, acceptor, inputProps, 'Input property');
        this.acceptList(context, acceptor, contextProps, 'Context property');
        this.acceptList(context, acceptor, promptParams, 'Prompt parameter');
    }

    private acceptList(context: CompletionContext, acceptor: CompletionAcceptor, values: string[], detail: string): void {
        const unique = Array.from(new Set(values));
        for (const value of unique) {
            const item: CompletionValueItem = {
                label: value,
                kind: 6,
                detail
            };
            acceptor(context, item);
        }
    }
    
    /**
     * Check if we're in an import path context
     */
    private isImportPathContext(context: CompletionContext): boolean {
        const node = context.node;
        if (!node) return false;
        
        // Check if we're inside a FileImport node at the importPath property
        let current: AstNode | undefined = node;
        while (current) {
            if (isFileImport(current)) {
                return true;
            }
            current = current.$container;
        }
        
        return false;
    }
    
    /**
     * Check if we're in an import symbol context (inside the braces)
     */
    private isImportSymbolContext(context: CompletionContext): boolean {
        const node = context.node;
        if (!node) return false;
        
        // Check if we're inside a NamedImports node
        let current: AstNode | undefined = node;
        while (current) {
            if (isNamedImports(current)) {
                return true;
            }
            current = current.$container;
        }
        
        return false;
    }

    /**
     * Provide completion for import paths
     */
    private async completeImportPath(context: CompletionContext, acceptor: CompletionAcceptor): Promise<void> {
        // Get all .agent files in the workspace
        const allDocuments = await this.workspaceLock.read(() => 
            Array.from(this.documents.all)
        );
        const currentDocUri = context.document?.uri;
        
        if (!currentDocUri) return;
        
        // Get the directory of the current file
        const currentDir = this.getDirectory(currentDocUri.fsPath);
        
        for (const doc of allDocuments) {
            // Skip the current file
            if (doc.uri.toString() === currentDocUri.toString()) {
                continue;
            }
            
            // Only suggest .agent files
            if (!doc.uri.fsPath.endsWith('.agent')) {
                continue;
            }
            
            // Calculate relative path
            const relativePath = this.getRelativePath(currentDir, doc.uri.fsPath);
            
            // Create completion item
            const item: CompletionValueItem = {
                label: relativePath,
                kind: 17, // CompletionItemKind.File
                detail: 'Import from file',
                sortText: relativePath,
                insertText: `"${relativePath}"`
            };
            
            acceptor(context, item);
        }
    }
    
    /**
     * Provide completion for imported symbols
     */
    private completeImportedSymbols(context: CompletionContext, acceptor: CompletionAcceptor): void {
        // Find the import statement
        let current: AstNode | undefined = context.node;
        let importNode: AstNode | undefined;
        
        while (current) {
            if (isFileImport(current)) {
                importNode = current;
                break;
            }
            current = current.$container;
        }
        
        if (!importNode || !isFileImport(importNode)) return;
        
        // Get the import path
        const importPath = importNode.importPath;
        if (!importPath) return;
        
        // Resolve the import path to get the target file
        const currentDocUri = context.document?.uri;
        if (!currentDocUri) return;
        
        // Query the index for exported symbols from the target file
        // This would require the UriResolver to resolve the path
        // For now, we'll use the default completion which already works through ScopeProvider
        
        // The ScopeProvider already handles this, so we can fall back to default
        super.completionFor(context, context.features[context.features.length - 1], acceptor);
    }
    
    /**
     * Get directory from file path
     */
    private getDirectory(filePath: string): string {
        const lastSlash = Math.max(filePath.lastIndexOf('/'), filePath.lastIndexOf('\\'));
        return lastSlash >= 0 ? filePath.substring(0, lastSlash) : '';
    }
    
    /**
     * Get relative path from one file to another
     */
    private getRelativePath(fromDir: string, toPath: string): string {
        // Simple relative path calculation
        // In production, you'd use a proper path library
        
        const toDir = this.getDirectory(toPath);
        const toFile = toPath.substring(toDir.length + 1);
        
        // Remove .agent extension for cleaner import
        const fileName = toFile.replace('.agent', '');
        
        // If in same directory
        if (fromDir === toDir) {
            return `./${fileName}`;
        }
        
        // If in parent directory
        if (toDir.startsWith(fromDir)) {
            const relative = toDir.substring(fromDir.length + 1);
            return `./${relative}/${fileName}`;
        }
        
        // If in sibling or parent directory
        return `../${fileName}`;
    }
}

export class AuwgentHoverProvider extends AstNodeHoverProvider {
    constructor(services: LangiumServices) {
        super(services);
    }

    override async getHoverContent(document: any, params: any): Promise<Hover | undefined> {
        const templateHover = this.getTemplateInterpolationHover(document, params);
        if (templateHover) return templateHover;
        return super.getHoverContent(document, params);
    }

    protected getAstNodeHoverContent(): string | undefined {
        return undefined;
    }

    private getTemplateInterpolationHover(document: any, params: any): Hover | undefined {
        const textDocument = document?.textDocument;
        if (!textDocument) return undefined;
        const offset = textDocument.offsetAt(params.position);
        const container = this.getMultilineStringAtOffset(document, offset);
        if (!container) return undefined;
        const cstNode = container.$cstNode;
        if (!cstNode) return undefined;
        const fullText = textDocument.getText();
        const nodeText = fullText.slice(cstNode.offset, cstNode.offset + cstNode.length);
        const localOffset = offset - cstNode.offset;
        const openIndex = nodeText.lastIndexOf('{{', localOffset);
        if (openIndex === -1) return undefined;
        const closeIndex = nodeText.indexOf('}}', openIndex + 2);
        if (closeIndex === -1 || localOffset > closeIndex + 2) return undefined;
        const rawExpr = nodeText.slice(openIndex + 2, closeIndex);
        const expr = rawExpr.trim();
        if (!expr) return undefined;
        if (!/^[_a-zA-Z][\w_]*(\.[_a-zA-Z][\w_]*)*$/.test(expr)) return undefined;
        const parts = expr.split('.');
        const root = parts[0];
        const rest = parts.slice(1);
        const inputProps = getInputPropertyMap(container);
        const contextProps = getContextPropertyMap(container);
        const promptParams = getPromptParamMap(container);
        if (root === 'input') {
            const typeText = resolveHoverTypeForPath(inputProps, rest, buildObjectTypeFromConfigs(inputProps));
            if (!typeText) return undefined;
            return { contents: { kind: 'markdown', value: `input${rest.length ? '.' + rest.join('.') : ''}: ${typeText}` } };
        }
        if (root === 'ctx') {
            const typeText = resolveHoverTypeForPath(contextProps, rest, buildObjectTypeFromConfigs(contextProps));
            if (!typeText) return undefined;
            return { contents: { kind: 'markdown', value: `ctx${rest.length ? '.' + rest.join('.') : ''}: ${typeText}` } };
        }
        if (promptParams.has(root)) {
            const param = promptParams.get(root);
            if (!param?.t) return undefined;
            const typeText = resolveHoverTypeForPath(promptParams, rest, formatTypes(param.t));
            if (!typeText) return undefined;
            return { contents: { kind: 'markdown', value: `${root}${rest.length ? '.' + rest.join('.') : ''}: ${typeText}` } };
        }
        return undefined;
    }

    private getMultilineStringAtOffset(document: any, offset: number): MultilineStringLiteral | undefined {
        const root = document.parseResult?.value as Model | undefined;
        if (!root) return undefined;
        let best: MultilineStringLiteral | undefined;
        for (const node of AstUtils.streamAllContents(root)) {
            if (!isMultilineStringLiteral(node)) continue;
            const cstNode = node.$cstNode;
            if (!cstNode) continue;
            const start = cstNode.offset;
            const end = cstNode.offset + cstNode.length;
            if (offset >= start && offset <= end) {
                if (!best || cstNode.length < (best.$cstNode?.length ?? Infinity)) {
                    best = node;
                }
            }
        }
        return best;
    }
}

const getInputPropertiesInScope = (node?: AstNode): string[] => {
    const props = getPropertiesInScope(node, 'input');
    return props.map(p => p.name);
};

const getContextPropertiesInScope = (node?: AstNode): string[] => {
    const props = getPropertiesInScope(node, 'context');
    return props.map(p => p.name);
};

const getPromptParamsInScope = (node?: AstNode): string[] => {
    const prompt = getPromptContainer(node);
    return prompt?.params?.map((p: any) => p.name) ?? [];
};

const getInputPropertyMap = (node?: AstNode): Map<string, TypeConfigDeclaration> => {
    return toPropertyMap(getPropertiesInScope(node, 'input'));
};

const getContextPropertyMap = (node?: AstNode): Map<string, TypeConfigDeclaration> => {
    return toPropertyMap(getPropertiesInScope(node, 'context'));
};

const getPromptParamMap = (node?: AstNode): Map<string, TypeConfigDeclaration> => {
    const prompt = getPromptContainer(node);
    const params = prompt?.params ?? [];
    return toPropertyMap(params);
};

const getPropertiesInScope = (node: AstNode | undefined, kind: 'input' | 'context'): TypeConfigDeclaration[] => {
    const direct = getPropertiesFromContainer(node, kind);
    if (direct.length) return direct;
    const prompt = getPromptContainer(node);
    if (!prompt) return [];
    return getPropertiesFromPromptUsages(prompt, kind);
};

const getPropertiesFromContainer = (node: AstNode | undefined, kind: 'input' | 'context'): TypeConfigDeclaration[] => {
    let current: AstNode | undefined = node;
    while (current) {
        if (isAgent(current) || isHelper(current)) {
            const configs = (current as any).configs ?? [];
            for (const config of configs) {
                if (kind === 'input' && isInputConfig(config)) {
                    return (config.inProperties ?? []) as TypeConfigDeclaration[];
                }
                if (kind === 'context' && isContextConfig(config)) {
                    return (config.contextProperties ?? []) as TypeConfigDeclaration[];
                }
            }
        }
        current = current.$container;
    }
    return [];
};

const getPromptContainer = (node?: AstNode): NamedPrompt | undefined => {
    let current: AstNode | undefined = node;
    while (current) {
        if (isNamedPrompt(current)) return current;
        current = current.$container;
    }
    return undefined;
};

const getPropertiesFromPromptUsages = (prompt: NamedPrompt, kind: 'input' | 'context'): TypeConfigDeclaration[] => {
    const document = AstUtils.getDocument(prompt);
    const root = document.parseResult?.value as Model | undefined;
    if (!root) return [];
    const collected = new Map<string, TypeConfigDeclaration>();
    for (const node of AstUtils.streamAllContents(root)) {
        if (isFunctionCall(node) && node.func?.ref === prompt) {
            const container = getAgentOrHelperContainer(node);
            if (container) collectProperties(container, kind, collected);
        }
        if (isPromptCall(node) && node.prompt?.ref === prompt) {
            const container = getAgentOrHelperContainer(node);
            if (container) collectProperties(container, kind, collected);
        }
    }
    return Array.from(collected.values());
};

const getAgentOrHelperContainer = (node: AstNode | undefined): AstNode | undefined => {
    let current: AstNode | undefined = node;
    while (current) {
        if (isAgent(current) || isHelper(current)) return current;
        current = current.$container;
    }
    return undefined;
};

const collectProperties = (container: AstNode, kind: 'input' | 'context', collected: Map<string, TypeConfigDeclaration>): void => {
    const configs = (container as any).configs ?? [];
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
};

const toPropertyMap = (properties: TypeConfigDeclaration[]): Map<string, TypeConfigDeclaration> => {
    const map = new Map<string, TypeConfigDeclaration>();
    for (const prop of properties) {
        map.set(prop.name, prop);
    }
    return map;
};

const buildObjectTypeFromConfigs = (props: Map<string, TypeConfigDeclaration>): string => {
    if (props.size === 0) return '{}';
    const entries = Array.from(props.values()).map(prop => `${prop.name}${prop.isOptional ? '?' : ''}: ${formatTypes(prop.t)}`);
    return `{ ${entries.join(', ')} }`;
};

const resolveHoverTypeForPath = (
    props: Map<string, TypeConfigDeclaration>,
    path: string[],
    defaultType: string
): string | undefined => {
    if (path.length === 0) return defaultType;
    const first = props.get(path[0]);
    if (!first) return undefined;
    const resolved = resolveTypePath(first.t, path.slice(1));
    return resolved ? formatTypes(resolved) : undefined;
};

const resolveTypePath = (type: Types, path: string[]): Types | undefined => {
    let current: Types | undefined = type;
    for (const segment of path) {
        if (!current) return undefined;
        const next = resolvePropertyType(current, segment);
        if (!next) return undefined;
        current = next;
    }
    return current;
};

const resolvePropertyType = (type: Types, prop: string): Types | undefined => {
    if (isArrayType(type)) {
        return undefined;
    }
    if (isBaseType(type)) {
        const concrete = type.type;
        if ((type as any).typeRef?.ref) {
            const decl = (type as any).typeRef?.ref as TypeDeclaration;
            const match = decl.types.find(p => p.name === prop);
            return match?.t;
        }
        if (concrete && isObjectType(concrete)) {
            const match = concrete.properties.find(p => p.name === prop);
            return match?.type;
        }
    }
    return undefined;
};

const formatTypes = (node: Types): string => {
    if (isArrayType(node)) {
        return `${formatBaseType(node.elementType)}[]`;
    }
    if (isBaseType(node)) {
        return formatBaseType(node);
    }
    return 'unknown';
};

const formatBaseType = (node: BaseType): string => {
    if ((node as any).typeRef?.ref) {
        return (node as any).typeRef.ref.name;
    }
    const concrete = node.type;
    if (!concrete) return 'unknown';
    if ((concrete as any).$type === 'StringType') return 'string';
    if ((concrete as any).$type === 'NumberType') return 'number';
    if ((concrete as any).$type === 'BooleanType') return 'boolean';
    if (isObjectType(concrete)) return formatObjectType(concrete);
    if (isUnionType(concrete)) return concrete.options.join(' | ');
    return 'unknown';
};

const formatObjectType = (node: any): string => {
    const props = (node.properties ?? []) as Array<{ name: string; type: Types; isOptional?: boolean }>;
    if (props.length === 0) return '{}';
    const fields = props.map(p => `${p.name}${p.isOptional ? '?' : ''}: ${formatTypes(p.type)}`);
    return `{ ${fields.join(', ')} }`;
};
