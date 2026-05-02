/* @ts-self-types="./auwgent_wasm_runtime.d.ts" */
import * as wasm from "./auwgent_wasm_runtime_bg.wasm";
import { __wbg_set_wasm } from "./auwgent_wasm_runtime_bg.js";

__wbg_set_wasm(wasm);
wasm.__wbindgen_start();
export {
    AuwgentWasm, IntoUnderlyingByteSource, IntoUnderlyingSink, IntoUnderlyingSource
} from "./auwgent_wasm_runtime_bg.js";
