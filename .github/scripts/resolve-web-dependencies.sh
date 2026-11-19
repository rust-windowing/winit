#!/usr/bin/env bash
set -eu

cargo update -p pin-project --precise 1.1.10
# The web bindings are published together and must be resolved together.
cargo update -p wasm-bindgen-futures -p web-sys -p js-sys -p wasm-bindgen -p tracing-web --precise 0.4.55
