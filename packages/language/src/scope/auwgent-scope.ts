import { AstNode, DefaultScopeProvider, ReferenceInfo, Scope, StreamScope, stream } from 'langium';
import { isAgent, isToolConfig, ToolFunction } from '../generated/ast.js';

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
                    if (isToolConfig(config) && config.tool) {
                        tools.push(config.tool);
                    }
                }
                break; // Found the agent, stop walking
            }
            current = current.$container;
        }

        return tools;
    }
}