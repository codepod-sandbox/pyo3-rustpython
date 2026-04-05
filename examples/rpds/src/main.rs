use rustpython_vm::InterpreterBuilder;

mod extension {
    include!("lib.rs");
}

fn main() {
    let builder = InterpreterBuilder::new();
    // rpds-py's #[pymodule] function is named rpds_py
    let module_def = extension::rpds_py_module_def(&builder.ctx);
    let interp = builder.add_native_module(module_def).build();

    let exit_code = interp.run(|vm| {
        vm.run_block_expr(
            vm.new_scope_with_builtins(),
            r#"
from rpds import HashTrieMap, HashTrieSet, List

m = HashTrieMap()
m2 = m.insert("key", "value")
print(f"map: {m2}")
print("rpds basic test passed!")
"#,
        )
        .map(|_| ())
    });

    if exit_code != 0 {
        std::process::exit(exit_code as i32);
    }
}
