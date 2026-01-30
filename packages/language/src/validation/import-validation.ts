import type { ValidationAcceptor } from 'langium';
import { AstUtils, URI } from 'langium';
import type { FileImport, NamedImports, ImportSpecifier } from '../generated/ast.js';
import type { AuwgentServices } from '../auwgent-module.js';
import { AuwgentUriResolver } from '../auwgent-uri-resolver.js';

export class ImportValidation {
    private services: AuwgentServices | undefined;

    constructor(private uriResolver: AuwgentUriResolver) {}

    setServices(services: AuwgentServices): void {
        this.services = services;
    }

    checkImportStatement(importStmt: FileImport, accept: ValidationAcceptor): void {
        const document = AstUtils.getDocument(importStmt);
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

        if (importStmt.$type === 'NamedImports') {
            const namedImports = importStmt as NamedImports;
            for (const spec of namedImports.imports) {
                this.validateImportedSymbol(spec, targetUri, accept);
            }
        }
    }

    private validateImportedSymbol(
        spec: ImportSpecifier,
        targetUri: URI,
        accept: ValidationAcceptor
    ): void {
        if (!this.services) return;

        const symbolName = spec.imported.$refText;
        const expectedKind = spec.kind;
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

    private getExpectedTypeFromKind(kind: string): string {
        switch (kind) {
            case 'helper': return 'Helper';
            case 'type': return 'TypeDeclaration';
            case 'prompt': return 'NamedPrompt';
            default: return '';
        }
    }

    private getKindFromType(type: string): string {
        switch (type) {
            case 'Helper': return 'helper';
            case 'TypeDeclaration': return 'type';
            case 'NamedPrompt': return 'prompt';
            default: return 'unknown';
        }
    }
}
