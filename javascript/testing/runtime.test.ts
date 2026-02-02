import { describe, it, expect } from "bun:test";
import { Agent } from "../loader/IrInterpreter";
import { AgentTestRunner } from "./AgentTestRunner";
import type { AgentDriver, DriverResult, SyntheticRequest } from "../loader/types/protocol";
import type { AgentIR } from "../loader/types/ir";

describe("Agent runtime orchestration", () => {
    it("retains tool availability across multiple model turns", async () => {
        const driver = new SequencedDriver();
        const agent = new Agent<Record<string, any>, string>({ openai: driver });
        agent.load(buildTestAgent());

        const tools = {
            firstTool: async (args: Record<string, any>) => {
                const { step } = args as { step: number };
                return step + 1;
            },
            secondTool: async (args: Record<string, any>) => {
                const { flag } = args as { flag: boolean };
                return flag ? "complete" : "incomplete";
            }
        } satisfies Record<string, (args: Record<string, any>) => Promise<any> | any>;

        const result = await agent.run({ question: "start" }, { tools });

        expect(result).toBe("done");
        expect(driver.callCount).toBe(3);
        expect(driver.observedTools[1]?.map(tool => tool.name)).toContain("secondTool");
    });
});

describe("Agent test runner integration", () => {
    it("executes embedded agent tests and reports success", async () => {
        const agentIr = buildTestAgent({ includeTests: true });
        const runner = new AgentTestRunner(agentIr);
        const outcome = await runner.runTest("happy-path");

        expect(outcome.passed).toBe(true);
        expect(outcome.failures).toHaveLength(0);
    });
});

function buildTestAgent(options?: { includeTests?: boolean }): AgentIR {
    const includeTests = options?.includeTests ?? false;
    const base: AgentIR = {
        name: "TestAgent",
        modelConfig: [
            {
                defaultConfig: {
                    model: {
                        type: "openai",
                        modelName: "stub-model"
                    },
                    prompt: null
                },
                namedConfig: []
            }
        ],
        input: {
            question: {
                type: "string",
                optional: false
            }
        },
        output: {},
        context: {},
        tools: [
            {
                name: "firstTool",
                description: "First step tool",
                params: {
                    step: {
                        type: "number",
                        optional: false
                    }
                },
                returns: "number"
            },
            {
                name: "secondTool",
                description: "Second step tool",
                params: {
                    flag: {
                        type: "boolean",
                        optional: false
                    }
                },
                returns: "string"
            }
        ],
        workflows: [],
        helpers: [],
        types: {},
        tests: includeTests
            ? [
                {
                    name: "happy-path",
                    expectations: [
                        {
                            type: "output",
                            path: ["value"],
                            value: { type: "literal", value: 42 }
                        }
                    ],
                    model: {
                        finalText: "value: 42"
                    },
                    toolStubs: [
                        {
                            name: "firstTool",
                            returns: { type: "literal", value: 1 }
                        },
                        {
                            name: "secondTool",
                            returns: { type: "literal", value: "complete" }
                        }
                    ]
                }
            ]
            : []
    };

    return base;
}

class SequencedDriver implements AgentDriver {
    name = "openai";
    callCount = 0;
    observedTools: Array<SyntheticRequest["tools"]> = [];

    async execute(request: SyntheticRequest): Promise<DriverResult> {
        this.callCount += 1;
        this.observedTools.push(request.tools);

        if (this.callCount === 1) {
            return {
                toolCalls: [
                    {
                        id: "call-1",
                        name: "firstTool",
                        args: { step: 1 }
                    }
                ]
            };
        }

        if (this.callCount === 2) {
            if (!request.tools || request.tools.length < 2) {
                throw new Error("Tools missing on follow-up call");
            }
            return {
                toolCalls: [
                    {
                        id: "call-2",
                        name: "secondTool",
                        args: { flag: true }
                    }
                ]
            };
        }

        return {
            content: [
                {
                    type: "text",
                    text: "done"
                }
            ]
        };
    }
}

