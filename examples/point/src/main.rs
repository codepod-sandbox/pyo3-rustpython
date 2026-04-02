use rustpython_vm::InterpreterBuilder;

mod extension {
    include!("lib.rs");
}

fn main() {
    let builder = InterpreterBuilder::new();
    let module_def = extension::point_module_def(&builder.ctx);
    let interp = builder.add_native_module(module_def).build();

    let exit_code = interp.run(|vm| {
        vm.run_block_expr(
            vm.new_scope_with_builtins(),
            r#"
from point import Point

p = Point(3.0, 4.0)
assert repr(p) == "Point(3.0, 4.0)", f"repr: {repr(p)}"
assert str(p) == "(3.0, 4.0)", f"str: {str(p)}"
assert p.x == 3.0
assert p.y == 4.0
assert p.distance() == 5.0, f"distance: {p.distance()}"

p.x = 1.0
p.y = 1.0
assert p.x == 1.0

p2 = p.translate(2.0, 3.0)
assert p2.x == 3.0
assert p2.y == 4.0

print("All point tests passed!")
"#,
        )
        .map(|_| ())
    });

    if exit_code != 0 {
        std::process::exit(exit_code as i32);
    }
}
