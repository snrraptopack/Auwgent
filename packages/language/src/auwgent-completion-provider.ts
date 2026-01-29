import { CompletionAcceptor, CompletionContext, CompletionValueItem, DefaultCompletionProvider, NextFeature, LangiumServices } from 'langium/lsp';
import { AstNode } from 'langium';
import { isFileImport, isNamedImports } from './generated/ast.js';

/**
 * Custom completion provider for Auwgent
 * Provides autocomplete for import paths and imported symbols
 */
export class AuwgentCompletionProvider extends DefaultCompletionProvider {
    
    constructor(services: LangiumServices) {
        super(services);
    }
    
    override completionFor(context: CompletionContext, next: NextFeature, acceptor: CompletionAcceptor): void {
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
    private completeImportPath(context: CompletionContext, acceptor: CompletionAcceptor): void {
        // Get all .agent files in the workspace
        const allDocuments = this.workspaceLock.read(() => 
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
