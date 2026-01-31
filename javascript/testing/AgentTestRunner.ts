import { ExpressionEvaluator } from "../loader/ExpressionEvaluator";
import { Synthesizer } from "../loader/Synthesizer";
import { WorkflowRunner } from "../loader/WorkflowRunner";
import type { AgentIR, AgentTest, Expression, TestExpectation, TestToolCall, TestToolStub } from "../loader/types/ir";
import type { ContentBlock } from "../loader/types/protocol";
import type { ToolMap } from "../loader/types/tool";

type TestResult = {
    name: string;
    passed: boolean;
    failures: string[];
};

export class AgentTestRunner {
    constructor(private ir: AgentIR) { }

    async runTest(name: string): Promise<TestResult> {
        const test = this.ir.tests?.find(t => t.name === name);
        if (!test) {
            throw new Error(`Test not found: ${name}`);
        }

        const input = await this.resolveInput(test);
        const promptText = await this.resolvePromptText(test, input);
        const toolErrors: string[] = [];
        const toolMap = this.buildToolMap(test, input, toolErrors);
        const output = await this.executeModelScript(test, input, toolMap, toolErrors);
        const failures = await this.evaluateExpectations(test, input, output, promptText, toolErrors);
        return { name, passed: failures.length === 0, failures };
    }

    async runAllTests(): Promise<{ passed: boolean; results: TestResult[] }> {
        const tests = this.ir.tests ?? [];
        const results: TestResult[] = [];
        for (const test of tests) {
            results.push(await this.runTest(test.name));
        }
        return {
            passed: results.every(r => r.passed),
            results
        };
    }

    private async resolveInput(test: AgentTest): Promise<Record<string, any>> {
        if (!test.input) {
            return {};
        }
        const scope = this.createScope({}, {});
        const evaluator = new ExpressionEvaluator(this.ir);
        const value = await evaluator.evaluate(test.input, scope);
        if (value && typeof value === "object") {
            return value;
        }
        return {};
    }

    private async resolvePromptText(test: AgentTest, input: Record<string, any>): Promise<string> {
        const synthesizer = new Synthesizer(this.ir);
        const request = await synthesizer.synthesize(input, undefined, test.configName);
        return request.messages
            .map(message => this.flattenContent(message.content))
            .filter(Boolean)
            .join("\n");
    }

    private buildToolMap(test: AgentTest, input: Record<string, any>, toolErrors: string[]): ToolMap {
        const stubs = new Map<string, TestToolStub>();
        for (const stub of test.toolStubs ?? []) {
            stubs.set(stub.name, stub);
        }
        const tools: ToolMap = {};
        for (const [name, stub] of stubs.entries()) {
            tools[name] = async (args: Record<string, any>) => {
                if (stub.error) {
                    toolErrors.push(stub.error);
                    throw new Error(stub.error);
                }
                if (!stub.returns) {
                    return undefined;
                }
                const scope = this.createScope(input, { args, ...args });
                const evaluator = new ExpressionEvaluator(this.ir, tools);
                return evaluator.evaluate(stub.returns as Expression, scope);
            };
        }
        return tools;
    }

    private async executeModelScript(
        test: AgentTest,
        input: Record<string, any>,
        tools: ToolMap,
        toolErrors: string[]
    ): Promise<any> {
        if (!test.model) {
            return undefined;
        }
        const callResults: Record<string, any> = {};
        for (const call of test.model.toolCalls ?? []) {
            const result = await this.executeToolCall(call, input, tools, toolErrors);
            callResults[call.name] = result;
        }
        if (typeof test.model.finalText === "string") {
            return this.parseFinalOutput(test.model.finalText);
        }
        return undefined;
    }

    private async executeToolCall(
        call: TestToolCall,
        input: Record<string, any>,
        tools: ToolMap,
        toolErrors: string[]
    ): Promise<any> {
        const scope = this.createScope(input, {});
        const evaluator = new ExpressionEvaluator(this.ir, tools);
        const args = call.args ? await evaluator.evaluate(call.args, scope) : {};
        const workflow = this.ir.workflows?.find(w => w.flowName === call.name);
        if (workflow) {
            const runner = new WorkflowRunner(this.ir, tools);
            return runner.run(call.name, args ?? {}, {});
        }
        const tool = tools[call.name];
        if (!tool) {
            const error = `Missing tool stub: ${call.name}`;
            toolErrors.push(error);
            throw new Error(error);
        }
        try {
            return await tool(args ?? {});
        } catch (err: any) {
            toolErrors.push(err?.message ?? String(err));
            return { __toolError: true, message: err?.message ?? String(err) };
        }
    }

    private parseFinalOutput(text: string): any {
        try {
            return JSON.parse(text);
        } catch {
            return text;
        }
    }

    private async evaluateExpectations(
        test: AgentTest,
        input: Record<string, any>,
        output: any,
        promptText: string,
        toolErrors: string[]
    ): Promise<string[]> {
        const failures: string[] = [];
        for (const expectation of test.expectations ?? []) {
            const failure = await this.checkExpectation(expectation, input, output, promptText, toolErrors);
            if (failure) {
                failures.push(failure);
            }
        }
        return failures;
    }

    private async checkExpectation(
        expectation: TestExpectation,
        input: Record<string, any>,
        output: any,
        promptText: string,
        toolErrors: string[]
    ): Promise<string | null> {
        if (expectation.type === "prompt_contains") {
            if (!promptText.includes(expectation.contains)) {
                return `Expected prompt to contain: ${expectation.contains}`;
            }
            return null;
        }
        if (expectation.type === "tool_error") {
            const matched = toolErrors.some(err => err.includes(expectation.error));
            if (!matched) {
                return `Expected tool error to include: ${expectation.error}`;
            }
            return null;
        }
        if (expectation.type === "output") {
            if (output === undefined || output === null) {
                return `Expected output at path ${expectation.path.join(".")}, but output was empty`;
            }
            const actual = this.getPathValue(output, expectation.path);
            const scope = this.createScope(input, {});
            const evaluator = new ExpressionEvaluator(this.ir);
            const expected = await evaluator.evaluate(expectation.value, scope);
            if (!this.deepEqual(actual, expected)) {
                return `Expected output.${expectation.path.join(".")} to equal ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`;
            }
            return null;
        }
        return null;
    }

    private getPathValue(value: any, path: string[]): any {
        let current = value;
        for (const part of path) {
            if (current === null || current === undefined) {
                return undefined;
            }
            current = current[part];
        }
        return current;
    }

    private deepEqual(a: any, b: any): boolean {
        if (a === b) {
            return true;
        }
        return JSON.stringify(a) === JSON.stringify(b);
    }

    private createScope(input: Record<string, any>, extra: Record<string, any>): Map<string, any> {
        return new Map(Object.entries({ ...input, input, ctx: {}, ...extra }));
    }

    private flattenContent(content: ContentBlock[] | string): string {
        if (typeof content === "string") {
            return content;
        }
        return content.map(block => {
            if (block.type === "text") {
                return block.text;
            }
            if (block.type === "tool_result") {
                if (typeof block.content === "string") {
                    return block.content;
                }
            }
            return "";
        }).join("");
    }
}
