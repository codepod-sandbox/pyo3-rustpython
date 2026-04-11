// Drives a RustPython interpreter with the `hello` module registered,
// then runs a quick smoke test from Python.

use pyo3::interp::InterpreterBuilder;

mod extension {
    include!("lib.rs");
}

fn main() {
    let builder = InterpreterBuilder::new();
    let module_def = extension::hello_module_def(&builder.ctx);
    let interp = builder.add_native_module(module_def).build();

    let exit_code = interp.run(|vm| {
        vm.run_block_expr(
            vm.new_scope_with_builtins(),
            r#"
import hello
result = hello.greet("world")
assert result == "hello, world!", f"got: {result!r}"
print(result)
"#,
        )
        .map(|_| ())
    });

    if exit_code != 0 {
        std::process::exit(exit_code as i32);
    }
}
