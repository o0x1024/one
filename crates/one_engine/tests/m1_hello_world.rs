use one_engine::Engine;

#[test]
fn m1_console_log_hello_world() {
    let mut engine = Engine::new();
    let result = engine.eval(r#"console.log("Hello World");"#);
    assert!(result.is_ok(), "M1 failed: {:?}", result.err());
}

#[test]
fn m1_arithmetic() {
    let mut engine = Engine::new();
    let result = engine.eval("return 1 + 2 + 3;").unwrap();
    assert!(result.to_number() == 6.0);
}

#[test]
fn m1_variables_and_operations() {
    let mut engine = Engine::new();
    let result = engine.eval("let x = 10; let y = 32; return x + y;").unwrap();
    assert!(result.to_number() == 42.0);
}
