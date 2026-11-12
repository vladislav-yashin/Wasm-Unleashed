# The project's user-facing name, sourced by the remaining packaging and brand assets (the browser title is
# crates/open-unleashed/src/brand.rs). Change both together. It names no trademark of the original game's
# owners; the READMEs say which game it runs.
APP_ID=WasmUnleashed
APP_TITLE="Wasm Unleashed"
BUNDLE_ID=dev.wasmunleashed.WasmUnleashed
# @TITLE@ in a README template -> the title
brand_readme() { sed "s/@TITLE@/$APP_TITLE/g; s/@APP@/$APP_ID/g" "$1"; }
