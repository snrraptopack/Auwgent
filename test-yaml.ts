import { createManger } from "./ir-runtime/typescript/main.agent.types";
import { parse } from "./ir-runtime/typescript/index.js";

const yaml = `response_text:
  text: Hello Amihere Theophilus! Your full details are: ID: 300, Location: Ghana, Grades: A, B, C.`;

console.log(JSON.stringify(parse(yaml), null, 2));
