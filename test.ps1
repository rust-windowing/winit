cargo build --target wasm32-unknown-unknown --features web_sys --example window
wasm-bindgen .\target\wasm32-unknown-unknown\debug\examples\window.wasm --out-dir . --target web