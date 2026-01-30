import type { ValidationAcceptor } from 'langium';
import { AstUtils, URI } from 'langium';
import type { Model } from '../generated/ast.js';
import type { AuwgentServices } from '../auwgent-module.js';
import { AuwgentUriResolver } from '../auwgent-uri-resolver.js';

export class DependencyValidation {
    private services: AuwgentServices | undefined;

    constructor(private uriResolver: AuwgentUriResolver) {}

    setServices(services: AuwgentServices): void {
        this.services = services;
    }

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

    checkImportOrdering(model: Model, accept: ValidationAcceptor): void {
        const hasElements = model.elements.length > 0;
        if (hasElements && model.imports.length > 0) {
            return;
        }
    }

    private detectCycle(
        currentUri: string,
        visited: Set<string>,
        recursionStack: Set<string>,
        path: string[]
    ): string[] | null {
        if (recursionStack.has(currentUri)) {
            const cycleStart = path.indexOf(currentUri);
            return path.slice(cycleStart).concat(currentUri);
        }

        if (visited.has(currentUri)) {
            return null;
        }

        visited.add(currentUri);
        recursionStack.add(currentUri);
        path.push(this.getFileNameFromUri(currentUri));

        if (!this.services) return null;

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

    private getFileNameFromUri(uriString: string): string {
        const uri = URI.parse(uriString);
        const path = uri.path;
        const segments = path.split('/');
        return segments[segments.length - 1] || uriString;
    }
}
