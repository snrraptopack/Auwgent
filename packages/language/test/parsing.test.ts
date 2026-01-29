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

describe("Parsing tests", () => {
    it("parses workflow tools and comma-separated object types", async () => {
        const document = await parse(`
agent Shop {
  workflow flow_product(id:string):{success:boolean, reason:string}{
    description: "buy flow"
    tools {
      purchase_product(id:string, user_id:string):boolean
    }
    let result = purchase_product(id, "u1")
    return {success: result, reason: "ok"}
  }
}
        `);
        expect(document.parseResult.parserErrors.length).toBe(0);
        expect(document.diagnostics?.length ?? 0).toBe(0);
    });
});
