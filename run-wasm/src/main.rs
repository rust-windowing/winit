fn main() {
    cargo_run_wasm::run_wasm_with_css(
        r#"
        body { 
            margin: 0px; 
            display: flex;
            flex-direction: column;
            justify-content: center;
            align-items: center;
        }
        "#,
    );
}
