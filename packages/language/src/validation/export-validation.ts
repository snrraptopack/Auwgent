import type { ValidationAcceptor } from 'langium';
import type { Exportable } from '../generated/ast.js';
import { isHelper, isTypeDeclaration, isNamedPrompt, isModelDefinition } from '../generated/ast.js';

export class ExportValidation {
    checkExportDependencies(element: Exportable, accept: ValidationAcceptor): void {
        if (!element.exported) return;

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

    private getReferencedElements(element: Exportable): Exportable[] {
        const referenced: Exportable[] = [];

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

        if (isHelper(element)) {
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

    private isExportable(element: any): element is Exportable {
        return isHelper(element) || isTypeDeclaration(element) || isNamedPrompt(element) || isModelDefinition(element);
    }
}
