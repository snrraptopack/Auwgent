import type { ValidationAcceptor } from 'langium';
import type { WorkFlowConfig, NamedPrompt, ModelConfig, Agent, Helper, TestConfig } from '../generated/ast.js';
import { isAgent, isHelper } from '../generated/ast.js';
import { TypeChecker } from '../type-system/checker.js';

export class TypeCheckValidation {
    checkWorkflowTypes(workflow: WorkFlowConfig, accept: ValidationAcceptor): void {
        const model = this.getRootModel(workflow);
        if (!model) return;
        const checker = new TypeChecker(model);
        const issues = checker.checkWorkflow(workflow);
        for (const issue of issues) {
            accept('error', issue.message, { node: issue.node as any, property: issue.property });
        }
    }

    checkPromptTypes(prompt: NamedPrompt, accept: ValidationAcceptor): void {
        const model = this.getRootModel(prompt);
        if (!model) return;
        const checker = new TypeChecker(model);
        const issues = checker.checkPrompt(prompt);
        for (const issue of issues) {
            accept('error', issue.message, { node: issue.node as any, property: issue.property });
        }
    }

    checkModelConfigTypes(modelConfig: ModelConfig, accept: ValidationAcceptor): void {
        const model = this.getRootModel(modelConfig);
        if (!model) return;
        const container = this.getAgentContainer(modelConfig);
        const checker = new TypeChecker(model);
        const issues = checker.checkModelConfig(modelConfig, container);
        for (const issue of issues) {
            accept('error', issue.message, { node: issue.node as any, property: issue.property });
        }
    }

    checkTestConfigTypes(testConfig: TestConfig, accept: ValidationAcceptor): void {
        const model = this.getRootModel(testConfig);
        if (!model) return;
        const container = this.getAgentContainer(testConfig);
        const checker = new TypeChecker(model);
        const issues = checker.checkTestConfig(testConfig, container);
        for (const issue of issues) {
            accept('error', issue.message, { node: issue.node as any, property: issue.property });
        }
    }

    private getRootModel(node: any): any | undefined {
        let current = node as any;
        while (current) {
            if (current.$type === 'Model') return current;
            current = current.$container;
        }
        return undefined;
    }

    private getAgentContainer(node: any): Agent | Helper | undefined {
        let current = node as any;
        while (current) {
            if (isAgent(current) || isHelper(current)) {
                return current;
            }
            current = current.$container;
        }
        return undefined;
    }
}
