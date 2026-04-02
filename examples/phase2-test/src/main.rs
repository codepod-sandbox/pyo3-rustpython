use rustpython_vm::InterpreterBuilder;

mod extension {
    include!("lib.rs");
}

fn main() {
    let builder = InterpreterBuilder::new();
    let module_def = extension::phase2_module_def(&builder.ctx);
    let interp = builder.add_native_module(module_def).build();

    let exit_code = interp.run(|vm| {
        vm.run_block_expr(
            vm.new_scope_with_builtins(),
            r#"
from phase2 import Converter

c = Converter("test")
assert repr(c) == "Converter('test')"
assert c.label == "test"

# Type conversions
assert c.extract_int(21) == 42
assert c.extract_float(1.5) == 2.0
assert c.extract_bool(True) == False
assert c.extract_bool(False) == True
assert c.extract_string("hello") == "test:hello"

# Error handling with exceptions
assert c.validate(50) == "valid: 50"

try:
    c.validate(-1)
    assert False, "should have raised ValueError"
except ValueError as e:
    assert "non-negative" in str(e)

try:
    c.validate(200)
    assert False, "should have raised TypeError"
except TypeError as e:
    assert "<= 100" in str(e)

# Test basic return
assert c.double(21) == 42

print("All phase 2 tests passed!")
"#,
        )
        .map(|_| ())
    });

    if exit_code != 0 {
        std::process::exit(exit_code as i32);
    }
}
