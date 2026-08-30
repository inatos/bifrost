"""Post-Trunk index.html fixes: wasmBindings before init + strip fragile SRI.

Trunk embeds sha384 integrity on modulepreload/wasm/css. Immutable long-cache
on those URLs plus mid-deploy HTML/JS skew (or a stale browser body for the
same hashed filename) makes Firefox reject the WASM glue with an integrity
mismatch — WASM never starts, so Matchbox never connects and Ready Up never
appears. Strip integrity so load follows the live bytes.
"""
import re
from pathlib import Path

path = Path("dist/index.html")
html = path.read_text()

# Drop Subresource Integrity — hashed filenames already cache-bust content.
html = re.sub(r'\s+integrity="[^"]+"', "", html)

module_re = re.compile(r"<script type=\"module\">.*?</script>", re.DOTALL)
match = module_re.search(html)
if not match:
    raise SystemExit("patch_wasm_bindings: no module script found in dist/index.html")

module = match.group(0)
html = html.replace(module, "", 1)

wasm_path = re.search(r"module_or_path: ('[^']+'|\"[^\"]+\")", module)
if not wasm_path:
    raise SystemExit("patch_wasm_bindings: could not find wasm module path")

wasm_ref = wasm_path.group(1)
js_import = re.search(
    r"import init, \* as bindings from ('[^']+'|\"[^\"]+\")",
    module,
)
if not js_import:
    raise SystemExit("patch_wasm_bindings: could not find wasm-bindgen import")

js_ref = js_import.group(1)

module = (
    "<script type=\"module\">\n"
    f"import init, * as bindings from {js_ref};\n"
    "window.wasmBindings = bindings;\n"
    f"init({{ module_or_path: {wasm_ref} }}).then((wasm) => {{\n"
    "const ev = new CustomEvent(\"TrunkApplicationStarted\", {detail: {wasm}});\n"
    "window.dispatchEvent(ev);\n"
    "document.dispatchEvent(ev);\n"
    "}).catch((err) => {\n"
    "console.error(\"[bifrost] wasm init failed\", err);\n"
    "const fail = new CustomEvent(\"TrunkApplicationFailed\", {detail: {err}});\n"
    "window.dispatchEvent(fail);\n"
    "document.dispatchEvent(fail);\n"
    "});\n"
    "</script>"
)

html = html.replace("</body>", module + "\n</body>", 1)
path.write_text(html)
