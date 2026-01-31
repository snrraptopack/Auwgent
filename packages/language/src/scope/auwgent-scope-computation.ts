import { AstNode, AstNodeDescription, DefaultScopeComputation, LangiumDocument } from 'langium';
import { isHelper, isTypeDeclaration, isNamedPrompt, isModelDefinition, Model, Exportable } from '../generated/ast.js';

/**
 * Custom scope computation for Auwgent that handles export collection
 */
export class AuwgentScopeComputation extends DefaultScopeComputation {
    
    /**
     * Collects exported symbols from a document for the global scope
     * Only elements marked with 'export' keyword are included
     */
    async computeExports(document: LangiumDocument): Promise<AstNodeDescription[]> {
        const exportedDescriptions: AstNodeDescription[] = [];
        const model = document.parseResult.value as Model;
        
        if (!model || !model.elements) {
            return exportedDescriptions;
        }
        
        // Collect all exportable elements that are marked as exported
        for (const element of model.elements) {
            if (this.isExportable(element) && element.exported) {
                const description = this.descriptions.createDescription(
                    element,
                    element.name,
                    document
                );
                exportedDescriptions.push(description);
            }
        }
        
        return exportedDescriptions;
    }
    
    /**
     * Type guard to check if an element is exportable
     */
    private isExportable(element: AstNode): element is Exportable {
        return isHelper(element) || isTypeDeclaration(element) || isNamedPrompt(element) || isModelDefinition(element);
    }
}
