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

describe("Validating", () => {
    it("rejects inline prompt blocks in return statements", async () => {
        const document = await parse(`
agent Validate {
  workflow w():{success:boolean}{
    description: "w"
    return {{success:false}}
  }
}
        `, { validation: true });
        const errors = document.diagnostics?.filter(d => d.severity === 1) ?? [];
        expect(errors.length).toBeGreaterThan(0);
    });
});
