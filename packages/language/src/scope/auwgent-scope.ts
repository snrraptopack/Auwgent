import { AstNode, DefaultScopeProvider, ReferenceInfo, Scope, StreamScope, stream, AstNodeDescription, AstUtils, LangiumCoreServices } from 'langium';
import { isAgent, isContextConfig, isContextReference, isHelpersConfig, isHelperCall, isToolConfig, isToolsConfig, isWorkFlowConfig, Helper, ToolFunction, TypeConfigDeclaration, Model, isHelper, isTypeDeclaration, isNamedPrompt, isModelDefinition } from '../generated/ast.js';
import { AuwgentUriResolver } from '../auwgent-uri-resolver.js';

export class AuwgentScopeProvider extends DefaultScopeProvider {
    private uriResolver: AuwgentUriResolver;

    constructor(services: LangiumCoreServices) {
        super(services);
        this.uriResolver = new AuwgentUriResolver();
    }

    override getScope(context: ReferenceInfo): Scope {
        // Handle references to importable symbols (Helper, TypeDeclaration, NamedPrompt)
        if (this.isImportableReference(context)) {
            return this.getImportedScope(context);
        }

        // Handle FunctionCall.func references
        if (context.property === 'func') {
            // Find all ToolFunctions in the containing Agent
            const tools = this.getToolFunctionsInScope(context.container);

            if (tools.length > 0) {
                // Create descriptions for each tool
                const descriptions = tools.map(tool =>
                    this.descriptions.createDescription(tool, tool.name)
                );
                // Return as scope, with default scope as fallback
                return new StreamScope(stream(descriptions), super.getScope(context));
            }
        }

        // Handle ContextReference.property
        if (context.property === 'property' && isContextReference(context.container)) {
            const contextProps = this.getContextPropertiesInScope(context.container);
            if (contextProps.length > 0) {
                const descriptions = contextProps.map(prop =>
                    this.descriptions.createDescription(prop, prop.name)
                );
                return new StreamScope(stream(descriptions));
            }
        }

        // Handle HelperCall.helper
        if (context.property === 'helper' && isHelperCall(context.container)) {
            const helpers = this.getHelpersInScope(context.container);
            if (helpers.length > 0) {
                const descriptions = helpers.map(helper =>
                    this.descriptions.createDescription(helper, helper.name)
                );
                return new StreamScope(stream(descriptions));
            }
        }

        // Handle HelperRef.grantedTools - resolve tool references in "with tools { ... }"
        if (context.property === 'grantedTools') {
            const tools = this.getToolFunctionsInScope(context.container);
            if (tools.length > 0) {
                const descriptions = tools.map(tool =>
                    this.descriptions.createDescription(tool, tool.name)
                );
                return new StreamScope(stream(descriptions), super.getScope(context));
            }
        }

        return super.getScope(context);
    }

    /**
     * Check if this reference is to an importable type
     */
    private isImportableReference(context: ReferenceInfo): boolean {
        const refType = this.reflection.getReferenceType(context);
        return refType === 'Helper' || refType === 'TypeDeclaration' || refType === 'NamedPrompt' || refType === 'ModelDefinition';
    }

    /**
     * Get scope including both local and imported symbols
     */
    private getImportedScope(context: ReferenceInfo): Scope {
        const document = AstUtils.getDocument(context.container);
        const model = document.parseResult.value as Model;
        
        // Collect local scope (current file)
        const localDescriptions: AstNodeDescription[] = [];
        for (const element of model.elements) {
            if (isHelper(element) || isTypeDeclaration(element) || isNamedPrompt(element) || isModelDefinition(element)) {
                localDescriptions.push(
                    this.descriptions.createDescription(element, element.name, document)
                );
            }
        }
        
        // Collect imported scope
        const importedDescriptions: AstNodeDescription[] = [];
        
        for (const importStmt of model.imports) {
            const targetUri = this.uriResolver.resolveImportUri(
                importStmt.importPath,
                document.uri
            );
            
            if (!targetUri) continue;
            
            // Handle wildcard imports
            if (importStmt.$type === 'WildcardImport') {
                const namespace = (importStmt as any).namespace;
                const exports = this.indexManager.allElements(
                    this.getExportableType(context),
                    new Set([targetUri.toString()])
                );
                
                for (const exp of exports) {
                    const qualifiedName = `${namespace}.${exp.name}`;
                    importedDescriptions.push({
                        ...exp,
                        name: qualifiedName
                    });
                }
            }
            // Handle named imports
            else if (importStmt.$type === 'NamedImports') {
                const namedImports = (importStmt as any).imports;
                for (const spec of namedImports) {
                    const symbolName = spec.imported.$refText;
                    const localName = spec.alias || symbolName;
                    
                    const exports = this.indexManager.allElements(
                        this.getExportableType(context),
                        new Set([targetUri.toString()])
                    );
                    
                    const matchingExport = Array.from(exports).find(e => e.name === symbolName);
                    if (matchingExport) {
                        importedDescriptions.push({
                            ...matchingExport,
                            name: localName
                        });
                    }
                }
            }
        }
        
        // Combine local and imported scopes (local takes precedence)
        return this.createScope(importedDescriptions, this.createScope(localDescriptions));
    }

    /**
     * Get the exportable type name for querying the index
     */
    private getExportableType(context: ReferenceInfo): string {
        const refType = this.reflection.getReferenceType(context);
        return refType;
    }

    /**
     * Find all ToolFunction nodes by walking up to the parent Agent
     * and collecting from its configs.
     */
    private getToolFunctionsInScope(node: AstNode): ToolFunction[] {
        const tools: ToolFunction[] = [];

        // Walk up to find workflow then agent
        let current: AstNode | undefined = node;
        while (current) {
            if (isWorkFlowConfig(current)) {
                if (current.workflowToolConfigs) {
                    for (const config of current.workflowToolConfigs) {
                        if (config.tool) {
                            tools.push(config.tool);
                        }
                    }
                }
                if (current.workflowToolsConfigs) {
                    for (const config of current.workflowToolsConfigs) {
                        if (config.tools) {
                            tools.push(...config.tools);
                        }
                    }
                }
            }
            if (isAgent(current)) {
                // Found the agent - collect all ToolFunctions
                for (const config of current.configs) {
                    // Single tool: tool functionName()
                    if (isToolConfig(config) && config.tool) {
                        tools.push(config.tool);
                    }
                    // Grouped tools: tools { ... }
                    if (isToolsConfig(config) && config.tools) {
                        tools.push(...config.tools);
                    }
                }
                break; // Found the agent, stop walking
            }
            current = current.$container;
        }

        return tools;
    }

    private getContextPropertiesInScope(node: AstNode): TypeConfigDeclaration[] {
        let current: AstNode | undefined = node;
        while (current) {
            if (isAgent(current)) {
                for (const config of current.configs) {
                    if (isContextConfig(config)) {
                        return config.contextProperties;  // Only context props!
                    }
                }
            }
            current = current.$container;
        }
        return [];
    }

    /**
     * Find all Helper references declared in the HelpersConfig of the containing Agent
     */
    private getHelpersInScope(node: AstNode): Helper[] {
        const helpers: Helper[] = [];
        let current: AstNode | undefined = node;

        while (current) {
            if (isAgent(current)) {
                for (const config of current.configs) {
                    if (isHelpersConfig(config) && config.helpers) {
                        // Collect resolved helper references
                        for (const helperRef of config.helpers) {
                            if (helperRef.helper?.ref) {
                                helpers.push(helperRef.helper.ref);
                            }
                        }
                    }
                }
                break;
            }
            current = current.$container;
        }

        return helpers;
    }
} 
