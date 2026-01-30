import { CompletionAcceptor, CompletionContext, CompletionValueItem, DefaultCompletionProvider, NextFeature, LangiumServices } from 'langium/lsp';
import { AstNode } from 'langium';
import { isFileImport, isNamedImports, isAgent, isHelper, isInputConfig, isContextConfig, isNamedPrompt } from './generated/ast.js';

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

        const inputProps = this.getInputPropertiesInScope(context.node);
        const contextProps = this.getContextPropertiesInScope(context.node);
        const promptParams = this.getPromptParamsInScope(context.node);

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

    private getInputPropertiesInScope(node?: AstNode): string[] {
        let current: AstNode | undefined = node;
        while (current) {
            if (isAgent(current) || isHelper(current)) {
                const configs = (current as any).configs ?? [];
                for (const config of configs) {
                    if (isInputConfig(config)) {
                        return (config.inProperties ?? []).map(p => p.name);
                    }
                }
            }
            current = current.$container;
        }
        return [];
    }

    private getContextPropertiesInScope(node?: AstNode): string[] {
        let current: AstNode | undefined = node;
        while (current) {
            if (isAgent(current) || isHelper(current)) {
                const configs = (current as any).configs ?? [];
                for (const config of configs) {
                    if (isContextConfig(config)) {
                        return (config.contextProperties ?? []).map(p => p.name);
                    }
                }
            }
            current = current.$container;
        }
        return [];
    }

    private getPromptParamsInScope(node?: AstNode): string[] {
        let current: AstNode | undefined = node;
        while (current) {
            if (isNamedPrompt(current)) {
                return (current as any).params?.map((p: any) => p.name) ?? [];
            }
            current = current.$container;
        }
        return [];
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
