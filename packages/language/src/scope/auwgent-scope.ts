import { AstNode, DefaultScopeProvider, ReferenceInfo, Scope, StreamScope, stream } from 'langium';
import { isAgent, isContextConfig, isContextReference, isHelpersConfig, isHelperCall, isToolConfig, isToolsConfig, Helper, ToolFunction, TypeConfigDeclaration } from '../generated/ast.js';

export class AuwgentScopeProvider extends DefaultScopeProvider {

    override getScope(context: ReferenceInfo): Scope {
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

        return super.getScope(context);
    }

    /**
     * Find all ToolFunction nodes by walking up to the parent Agent
     * and collecting from its configs.
     */
    private getToolFunctionsInScope(node: AstNode): ToolFunction[] {
        const tools: ToolFunction[] = [];

        // Walk up to find the Agent
        let current: AstNode | undefined = node;
        while (current) {
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