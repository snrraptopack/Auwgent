import { beforeAll, describe, expect, it } from "vitest";
import { EmptyFileSystem } from "langium";
import { parseHelper } from "langium/test";
import type { Model } from "../src/generated/ast.js";
import { createAuwgentServices } from "../src/auwgent-module.js";

let services: ReturnType<typeof createAuwgentServices>;
let parse: ReturnType<typeof parseHelper<Model>>;

beforeAll(() => {
    services = createAuwgentServices(EmptyFileSystem);
    parse = parseHelper<Model>(services.Auwgent);
});

describe("Linking tests", () => {
    it("limits workflow tools to their workflow scope", async () => {
        const document = await parse(`
agent Scoped {
  workflow w1():string{
    description: "w1"
    tool inner():string
    return inner()
  }
  workflow w2():string{
    description: "w2"
    return inner()
  }
}
        `, { validation: true });
        const errors = document.diagnostics?.filter(d => d.severity === 1) ?? [];
        expect(errors.length).toBeGreaterThan(0);
    });
});
