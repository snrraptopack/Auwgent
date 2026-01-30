import type { ValidationAcceptor } from 'langium';
import type { ToolFunction, WorkFlowConfig, Statement, IfStatement } from '../generated/ast.js';
import { isIfStatement, isReturnStatement } from '../generated/ast.js';

export class ReturnTypeValidation {
    checkToolReturn(tool: ToolFunction, accept: ValidationAcceptor): void {
        if (!tool.returns) {
            accept('error', `Tool '${tool.name}' must declare a return type`, { node: tool, property: 'returns' });
        }
    }

    checkWorkflowReturn(workflow: WorkFlowConfig, accept: ValidationAcceptor): void {
        if (!workflow.return) {
            accept('error', `Workflow '${workflow.name}' must declare a return type`, { node: workflow, property: 'return' });
        }
        if (!this.hasReturnInStatements(workflow.body)) {
            accept('error', `Workflow '${workflow.name}' must return a value`, { node: workflow, property: 'body' });
        }
    }

    private hasReturnInStatements(statements: Statement[]): boolean {
        for (const statement of statements) {
            if (isReturnStatement(statement)) {
                return true;
            }
            if (isIfStatement(statement)) {
                if (this.hasReturnInIf(statement)) {
                    return true;
                }
            }
        }
        return false;
    }

    private hasReturnInIf(statement: IfStatement): boolean {
        if (this.hasReturnInStatements(statement.thenBlock)) {
            return true;
        }
        if (statement.elseBlock && this.hasReturnInStatements(statement.elseBlock)) {
            return true;
        }
        return false;
    }
}
