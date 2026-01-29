import type { ValidationAcceptor, ValidationChecks } from 'langium';
import { AstUtils, URI } from 'langium';
import type { AuwgentAstType, ReturnStatement, FileImport, NamedImports, ImportSpecifier, Model, Exportable } from './generated/ast.js';
import { isInlinePromptBlock, isHelper, isTypeDeclaration, isNamedPrompt } from './generated/ast.js';
import type { AuwgentServices } from './auwgent-module.js';
import { AuwgentUriResolver } from './auwgent-uri-resolver.js';

/**
 * Register custom validation checks.
 */
export function registerValidationChecks(services: AuwgentServices) {
    const registry = services.validation.ValidationRegistry;
    const validator = services.validation.AuwgentValidator;
    const checks: ValidationChecks<AuwgentAstType> = {
        ReturnStatement: validator.checkReturnStatement,
        FileImport: validator.checkImportStatement,
        Model: [validator.checkCircularDependencies, validator.checkImportOrdering],
        Helper: validator.checkExportDependencies,
        TypeDeclaration: validator.checkExportDependencies,
        NamedPrompt: validator.checkExportDependencies
    };
    registry.register(checks, validator);
}

/**
 * Implementation of custom validations.
 */
export class AuwgentValidator {
    private uriResolver: AuwgentUriResolver;
    private services: AuwgentServices | undefined;

    constructor() {
        this.uriResolver = new AuwgentUriResolver();
    }

    setServices(services: AuwgentServices): void {
        this.services = services;
    }

    checkReturnStatement(statement: ReturnStatement, accept: ValidationAcceptor): void {
        if (isInlinePromptBlock(statement.value)) {
            accept('error', 'Inline prompt blocks are not allowed in return statements. Use an object literal instead.', { node: statement, property: 'value' });
        }
    }


    /**
     * Validates import statements
     */
    checkImportStatement(importStmt: FileImport, accept: ValidationAcceptor): void {
        const document = AstUtils.getDocument(importStmt);
        
        // Validate import path resolution
        const targetUri = this.uriResolver.resolveImportUri(
            importStmt.importPath,
            document.uri
        );
        
        if (!targetUri) {
            accept('error', `Cannot resolve import path: ${importStmt.importPath}`, {
                node: importStmt,
                property: 'importPath'
            });
            return;
        }

        // Validate imported symbols exist and are exported (for named imports)
        if (importStmt.$type === 'NamedImports') {
            const namedImports = importStmt as NamedImports;
            for (const spec of namedImports.imports) {
                this.validateImportedSymbol(spec, targetUri, accept);
            }
        }
    }

    /**
     * Validates that imported symbols exist and are exported
     */
    private validateImportedSymbol(
        spec: ImportSpecifier,
        targetUri: URI,
        accept: ValidationAcceptor
    ): void {
        if (!this.services) return;

        const symbolName = spec.imported.$refText;
        const expectedKind = spec.kind; // 'helper', 'type', or 'prompt'
        
        // Query the index for exported symbols from the target file
        const indexManager = this.services.shared.workspace.IndexManager;
        const allExports = indexManager.allElements().filter(desc => 
            desc.documentUri.toString() === targetUri.toString()
        );
        
        const matchingExport = allExports.find(e => e.name === symbolName);
        
        if (!matchingExport) {
            const availableExports = allExports.map(e => e.name).join(', ');
            accept('error', 
                `Symbol '${symbolName}' is not exported from the target file. ` +
                `Available exports: ${availableExports || 'none'}`, {
                node: spec,
                property: 'imported'
            });
            return;
        }

        // Validate that the import kind matches the actual type (if kind is specified)
        if (expectedKind) {
            const actualType = matchingExport.type;
            const expectedType = this.getExpectedTypeFromKind(expectedKind);
            
            if (actualType !== expectedType) {
                const actualKind = this.getKindFromType(actualType);
                accept('error', 
                    `Import kind mismatch: '${symbolName}' is a ${actualKind}, not a ${expectedKind}`, {
                    node: spec,
                    property: 'kind'
                });
            }
        }
    }

    /**
     * Maps import kind to AST type
     */
    private getExpectedTypeFromKind(kind: string): string {
        switch (kind) {
            case 'helper': return 'Helper';
            case 'type': return 'TypeDeclaration';
            case 'prompt': return 'NamedPrompt';
            default: return '';
        }
    }

    /**
     * Maps AST type to import kind
     */
    private getKindFromType(type: string): string {
        switch (type) {
            case 'Helper': return 'helper';
            case 'TypeDeclaration': return 'type';
            case 'NamedPrompt': return 'prompt';
            default: return 'unknown';
        }
    }

    /**
     * Detects circular dependencies between files
     */
    checkCircularDependencies(model: Model, accept: ValidationAcceptor): void {
        const document = AstUtils.getDocument(model);
        const visited = new Set<string>();
        const recursionStack = new Set<string>();
        
        const cycle = this.detectCycle(
            document.uri.toString(),
            visited,
            recursionStack,
            []
        );
        
        if (cycle) {
            accept('error', 
                `Circular dependency detected: ${cycle.join(' -> ')}`, {
                node: model,
                property: 'imports'
            });
        }
    }

    /**
     * Recursively detects cycles in the import graph
     */
    private detectCycle(
        currentUri: string,
        visited: Set<string>,
        recursionStack: Set<string>,
        path: string[]
    ): string[] | null {
        if (recursionStack.has(currentUri)) {
            // Found a cycle
            const cycleStart = path.indexOf(currentUri);
            return path.slice(cycleStart).concat(currentUri);
        }
        
        if (visited.has(currentUri)) {
            return null; // Already processed
        }
        
        visited.add(currentUri);
        recursionStack.add(currentUri);
        path.push(this.getFileNameFromUri(currentUri));
        
        if (!this.services) return null;

        // Get all imports from current file
        const documents = this.services.shared.workspace.LangiumDocuments;
        const document = documents.getDocument(URI.parse(currentUri));
        if (!document) return null;
        
        const model = document.parseResult.value as Model;
        
        for (const importStmt of model.imports) {
            const targetUri = this.uriResolver.resolveImportUri(
                importStmt.importPath,
                document.uri
            );
            
            if (targetUri) {
                const cycle = this.detectCycle(
                    targetUri.toString(),
                    visited,
                    recursionStack,
                    [...path]
                );
                
                if (cycle) return cycle;
            }
        }
        
        recursionStack.delete(currentUri);
        return null;
    }

    /**
     * Extracts filename from URI for display
     */
    private getFileNameFromUri(uriString: string): string {
        const uri = URI.parse(uriString);
        const path = uri.path;
        const segments = path.split('/');
        return segments[segments.length - 1] || uriString;
    }

    /**
     * Validates that imports appear before other elements
     */
    checkImportOrdering(model: Model, accept: ValidationAcceptor): void {
        // Check if any elements appear before imports
        const hasElements = model.elements.length > 0;
        
        if (hasElements && model.imports.length > 0) {
            // All imports should be at the top, this is already enforced by grammar
            // This check is redundant but kept for completeness
        }
    }

    /**
     * Validates export dependencies - warns if exported elements reference non-exported elements
     */
    checkExportDependencies(element: Exportable, accept: ValidationAcceptor): void {
        if (!element.exported) return;
        
        // Check if exported element references non-exported elements
        const referencedElements = this.getReferencedElements(element);
        
        for (const ref of referencedElements) {
            if (this.isExportable(ref) && !ref.exported) {
                accept('warning',
                    `Exported element '${element.name}' references non-exported ` +
                    `element '${ref.name}'. Consider exporting '${ref.name}' as well.`, {
                    node: element,
                    property: 'name'
                });
            }
        }
    }

    /**
     * Gets all elements referenced by an exportable element
     */
    private getReferencedElements(element: Exportable): Exportable[] {
        const referenced: Exportable[] = [];
        
        // For TypeDeclaration, check if it references other types
        if (isTypeDeclaration(element)) {
            for (const typeConfig of element.types) {
                const typeRef = typeConfig.t;
                if (typeRef.$type === 'BaseType' && typeRef.typeRef?.ref) {
                    const refElement = typeRef.typeRef.ref;
                    if (this.isExportable(refElement)) {
                        referenced.push(refElement);
                    }
                }
            }
        }
        
        // For Helper, check if it references other helpers or types
        if (isHelper(element)) {
            // Check input/output/context types
            for (const config of element.configs) {
                if (config.$type === 'InputConfig') {
                    for (const prop of config.inProperties) {
                        const typeRef = prop.t;
                        if (typeRef.$type === 'BaseType' && typeRef.typeRef?.ref) {
                            const refElement = typeRef.typeRef.ref;
                            if (this.isExportable(refElement)) {
                                referenced.push(refElement);
                            }
                        }
                    }
                }
                if (config.$type === 'OutputConfig') {
                    for (const output of config.outProperties) {
                        const typeRef = output.td.t;
                        if (typeRef.$type === 'BaseType' && typeRef.typeRef?.ref) {
                            const refElement = typeRef.typeRef.ref;
                            if (this.isExportable(refElement)) {
                                referenced.push(refElement);
                            }
                        }
                    }
                }
                if (config.$type === 'ContextConfig') {
                    for (const prop of config.contextProperties) {
                        const typeRef = prop.t;
                        if (typeRef.$type === 'BaseType' && typeRef.typeRef?.ref) {
                            const refElement = typeRef.typeRef.ref;
                            if (this.isExportable(refElement)) {
                                referenced.push(refElement);
                            }
                        }
                    }
                }
            }
        }
        
        return referenced;
    }

    /**
     * Type guard to check if an element is exportable
     */
    private isExportable(element: any): element is Exportable {
        return isHelper(element) || isTypeDeclaration(element) || isNamedPrompt(element);
    }
}
