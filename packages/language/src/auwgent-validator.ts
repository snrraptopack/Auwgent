import type { ValidationAcceptor, ValidationChecks } from 'langium';
import { AstUtils } from 'langium';
import type { AuwgentAstType, ReturnStatement, FileImport, Model, Exportable, MultilineStringLiteral, VariableRef, PromptCall, FunctionCall, ToolFunction, WorkFlowConfig, NamedPrompt, ModelConfig, TestConfig } from './generated/ast.js';
import { isAgent, isHelperCall, isHelpersConfig } from './generated/ast.js';
import type { AuwgentServices } from './auwgent-module.js';
import { AuwgentUriResolver } from './auwgent-uri-resolver.js';
import { ReturnValidation } from './validation/return-validation.js';
import { TemplateValidation } from './validation/template-validation.js';
import { ImportValidation } from './validation/import-validation.js';
import { DependencyValidation } from './validation/dependency-validation.js';
import { ExportValidation } from './validation/export-validation.js';
import { PromptUsageValidation } from './validation/prompt-usage-validation.js';
import { ReturnTypeValidation } from './validation/return-type-validation.js';
import { TypeCheckValidation } from './validation/type-check-validation.js';

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
        NamedPrompt: [validator.checkExportDependencies, validator.checkPromptTypes],
        MultilineStringLiteral: validator.checkTemplateInterpolations,
        VariableRef: validator.checkPromptRefUsage,
        PromptCall: validator.checkPromptCall,
        FunctionCall: validator.checkFunctionCall,
        ToolFunction: validator.checkToolReturn,
        WorkFlowConfig: [validator.checkWorkflowReturn, validator.checkWorkflowTypes, validator.checkWorkflowHelperUsage],
        ModelConfig: validator.checkModelConfigTypes,
        TestConfig: validator.checkTestConfigTypes
    };
    registry.register(checks, validator);
}

/**
 * Implementation of custom validations.
 */
export class AuwgentValidator {
    private uriResolver: AuwgentUriResolver;
    private returnValidation: ReturnValidation;
    private templateValidation: TemplateValidation;
    private importValidation: ImportValidation;
    private dependencyValidation: DependencyValidation;
    private exportValidation: ExportValidation;
    private promptUsageValidation: PromptUsageValidation;
    private returnTypeValidation: ReturnTypeValidation;
    private typeCheckValidation: TypeCheckValidation;

    constructor() {
        this.uriResolver = new AuwgentUriResolver();
        this.returnValidation = new ReturnValidation();
        this.templateValidation = new TemplateValidation();
        this.importValidation = new ImportValidation(this.uriResolver);
        this.dependencyValidation = new DependencyValidation(this.uriResolver);
        this.exportValidation = new ExportValidation();
        this.promptUsageValidation = new PromptUsageValidation();
        this.returnTypeValidation = new ReturnTypeValidation();
        this.typeCheckValidation = new TypeCheckValidation();
    }

    setServices(services: AuwgentServices): void {
        this.importValidation.setServices(services);
        this.dependencyValidation.setServices(services);
    }

    checkReturnStatement(statement: ReturnStatement, accept: ValidationAcceptor): void {
        this.returnValidation.checkReturnStatement(statement, accept);
    }

    checkTemplateInterpolations(node: MultilineStringLiteral, accept: ValidationAcceptor): void {
        this.templateValidation.checkTemplateInterpolations(node, accept);
    }


    /**
     * Validates import statements
     */
    checkImportStatement(importStmt: FileImport, accept: ValidationAcceptor): void {
        this.importValidation.checkImportStatement(importStmt, accept);
    }

    /**
     * Detects circular dependencies between files
     */
    checkCircularDependencies(model: Model, accept: ValidationAcceptor): void {
        this.dependencyValidation.checkCircularDependencies(model, accept);
    }

    /**
     * Validates that imports appear before other elements
     */
    checkImportOrdering(model: Model, accept: ValidationAcceptor): void {
        this.dependencyValidation.checkImportOrdering(model, accept);
    }

    /**
     * Validates export dependencies - warns if exported elements reference non-exported elements
     */
    checkExportDependencies(element: Exportable, accept: ValidationAcceptor): void {
        this.exportValidation.checkExportDependencies(element, accept);
    }

    checkPromptRefUsage(node: VariableRef, accept: ValidationAcceptor): void {
        this.promptUsageValidation.checkPromptRefUsage(node, accept);
    }

    checkPromptCall(node: PromptCall, accept: ValidationAcceptor): void {
        this.promptUsageValidation.checkPromptCall(node, accept);
    }

    checkFunctionCall(node: FunctionCall, accept: ValidationAcceptor): void {
        this.promptUsageValidation.checkFunctionCall(node, accept);
    }

    checkToolReturn(node: ToolFunction, accept: ValidationAcceptor): void {
        this.returnTypeValidation.checkToolReturn(node, accept);
    }

    checkWorkflowReturn(node: WorkFlowConfig, accept: ValidationAcceptor): void {
        this.returnTypeValidation.checkWorkflowReturn(node, accept);
    }

    checkWorkflowTypes(node: WorkFlowConfig, accept: ValidationAcceptor): void {
        this.typeCheckValidation.checkWorkflowTypes(node, accept);
    }

    checkWorkflowHelperUsage(node: WorkFlowConfig, accept: ValidationAcceptor): void {
        const container = this.getAgentContainer(node);
        if (!container) {
            return;
        }
        const allowedHelpers = new Set<string>();
        for (const config of container.configs) {
            if (isHelpersConfig(config)) {
                for (const helperRef of config.helpers) {
                    const name = helperRef.helper?.ref?.name;
                    if (name) {
                        allowedHelpers.add(name);
                    }
                }
            }
        }
        const stream = AstUtils.streamAst(node);
        for (const child of stream) {
            if (isHelperCall(child)) {
                const name = child.helper?.ref?.name;
                if (name && !allowedHelpers.has(name)) {
                    accept('error', `Helper "${name}" is not declared in helpers { }`, {
                        node: child,
                        property: 'helper'
                    });
                }
            }
        }
    }

    checkPromptTypes(node: NamedPrompt, accept: ValidationAcceptor): void {
        this.typeCheckValidation.checkPromptTypes(node, accept);
    }

    checkModelConfigTypes(node: ModelConfig, accept: ValidationAcceptor): void {
        this.typeCheckValidation.checkModelConfigTypes(node, accept);
    }

    checkTestConfigTypes(node: TestConfig, accept: ValidationAcceptor): void {
        this.typeCheckValidation.checkTestConfigTypes(node, accept);
    }

    private getAgentContainer(node: any): any | undefined {
        let current = node;
        while (current) {
            if (isAgent(current)) {
                return current;
            }
            current = current.$container;
        }
        return undefined;
    }
}
