use pyo3::interp::InterpreterBuilder;

mod py_lossless_float;
mod py_string_cache;
mod python;
mod jiter_module;

fn main() {
    let builder = InterpreterBuilder::new();
    let module_def = jiter_module::jiter_mod_module_def(&builder.ctx);
    let interp = builder.add_native_module(module_def).build();

    let exit_code = interp.run(|vm| {
        vm.run_block_expr(
            vm.new_scope_with_builtins(),
            r#"
from jiter_mod import from_json

# Basic JSON parsing
result = from_json(b'{"key": "value", "num": 42}')
assert isinstance(result, dict)
assert result["key"] == "value"
assert result["num"] == 42

# Arrays
arr = from_json(b'[1, 2, 3]')
assert arr == [1, 2, 3]

# Nested
nested = from_json(b'{"a": [1, {"b": true}]}')
assert nested["a"][1]["b"] == True

# Strings and numbers
assert from_json(b'"hello"') == "hello"
assert from_json(b'42') == 42
assert from_json(b'3.14') == 3.14
assert from_json(b'true') == True
assert from_json(b'false') == False
assert from_json(b'null') is None

print("All jiter tests passed!")
"#,
        )
        .map(|_| ())
    });

    if exit_code != 0 {
        std::process::exit(exit_code as i32);
    }
}
