use pyo3::interp::InterpreterBuilder;

mod extension {
    include!("lib.rs");
}

fn main() {
    let builder = InterpreterBuilder::new();
    let module_def = extension::rpds_py_module_def(&builder.ctx);
    let interp = builder.add_native_module(module_def).build();

    let exit_code = interp.run(|vm| {
        vm.run_block_expr(
            vm.new_scope_with_builtins(),
            r#"
from rpds_py import HashTrieMap

# Create an empty map
m = HashTrieMap()
print(f"created: {type(m).__name__}")

# insert and get
m2 = m.insert("hello", "world")
result = m2.get("hello")
assert result == "world", f"expected 'world', got {result}"
print(f"insert+get: hello -> {result}")

# Multiple inserts
m3 = m2.insert("foo", "bar")
result2 = m3.get("foo")
assert result2 == "bar"
print(f"second insert: foo -> {result2}")

# Original map unchanged (persistent!)
assert m2.get("foo") is None, "original map should not have 'foo'"
print("persistence verified")

print("All rpds tests passed!")
"#,
        )
        .map(|_| ())
    });

    if exit_code != 0 {
        std::process::exit(exit_code as i32);
    }
}
