use once_cell::sync::Lazy;
use rustpython::InterpreterBuilderExt;

static INIT: Lazy<()> = Lazy::new(|| {
    let _ = rustpython::InterpreterBuilder::new().init_stdlib().build();
});

#[allow(dead_code)]
pub fn ensure_runtime() {
    Lazy::force(&INIT);
}
