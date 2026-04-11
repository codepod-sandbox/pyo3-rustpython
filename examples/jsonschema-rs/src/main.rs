use jsonschema_rs::jsonschema_rs_module_def;
use pyo3::interp::InterpreterBuilder;
use rustpython_vm::{convert::IntoObject, AsObject};

fn main() {
    let builder = InterpreterBuilder::new();
    let module_def = jsonschema_rs_module_def(&builder.ctx);
    let interp = builder.add_native_module(module_def).init_stdlib().build();

    interp.enter(|vm| {
        let py = pyo3::Python::from_vm(vm);
        let mod_bound: pyo3::Bound<pyo3::types::PyModule> = py.import("jsonschema_rs").unwrap();

        let exc_type = vm.ctx.types.type_type.as_object();
        let exception_type = vm.ctx.exceptions.exception_type.as_object();

        let ve_name = vm.ctx.new_str("ValidationError");
        let ve_bases = vm.ctx.new_tuple(vec![exception_type.to_owned().into()]).into();
        let ve_dict = vm.ctx.new_dict().into();
        let ve_args = rustpython_vm::function::FuncArgs::new(
            vec![ve_name.into(), ve_bases, ve_dict],
            rustpython_vm::function::KwArgs::default(),
        );
        let ve_type = exc_type.call_with_args(ve_args, vm).unwrap();
        mod_bound.add("ValidationError", pyo3::Py::<pyo3::types::PyAny>::from_object(ve_type)).unwrap();

        let re_name = vm.ctx.new_str("ReferencingError");
        let re_bases = vm.ctx.new_tuple(vec![exception_type.to_owned().into()]).into();
        let re_dict = vm.ctx.new_dict().into();
        let re_args = rustpython_vm::function::FuncArgs::new(
            vec![re_name.into(), re_bases, re_dict],
            rustpython_vm::function::KwArgs::default(),
        );
        let re_type = exc_type.call_with_args(re_args, vm).unwrap();
        mod_bound.add("ReferencingError", pyo3::Py::<pyo3::types::PyAny>::from_object(re_type)).unwrap();

        let mut pass_count = 0;
        let mut fail_count = 0;

        macro_rules! run_test {
            ($name:expr, $code:expr) => {
                let full_code = format!("import jsonschema_rs\n{}", $code);
                let result = vm.run_block_expr(vm.new_scope_with_builtins(), &full_code);
                match result {
                    Ok(val) => {
                        let repr = val.into_object().repr(vm).unwrap().to_string();
                        println!("  PASS  {} => {}", $name, repr);
                        pass_count += 1;
                    }
                    Err(err) => {
                        let exc_repr = err.into_object().repr(vm).unwrap().to_string();
                        eprintln!("  FAIL  {} => {}", $name, exc_repr);
                        fail_count += 1;
                    }
                }
            };
        }

        // is_valid tests
        run_test!("is_valid_object", r#"jsonschema_rs.is_valid({"type": "object"}, {"name": "John"}) == True"#);
        run_test!("is_valid_string", r#"jsonschema_rs.is_valid({"type": "string"}, "hello") == True"#);
        run_test!("is_valid_number", r#"jsonschema_rs.is_valid({"type": "number"}, 42) == True"#);
        run_test!("is_valid_invalid", r#"jsonschema_rs.is_valid({"type": "string"}, 42) == False"#);
        run_test!("is_valid_boolean", r#"jsonschema_rs.is_valid({"type": "boolean"}, True) == True"#);
        run_test!("is_valid_null", r#"jsonschema_rs.is_valid({"type": "null"}, None) == True"#);
        run_test!("is_valid_array", r#"jsonschema_rs.is_valid({"type": "array"}, [1,2,3]) == True"#);

        // validate tests (should raise ValidationError for invalid)
        run_test!("validate_valid", r#"
try:
    jsonschema_rs.validate({"type": "string"}, "hello")
    True
except jsonschema_rs.ValidationError:
    False
"#);
        run_test!("validate_invalid", r#"
try:
    jsonschema_rs.validate({"type": "string"}, 42)
    False
except jsonschema_rs.ValidationError:
    True
"#);

        // validator_for tests
        run_test!("validator_for_type", r#"type(jsonschema_rs.validator_for({"type": "string"})).__name__"#);
        run_test!("validator_for_is_valid", r#"jsonschema_rs.validator_for({"type": "string"}).is_valid("hello") == True"#);
        run_test!("validator_for_draft7", r#"type(jsonschema_rs.validator_for({"type": "string"}, draft=7)).__name__"#);
        run_test!("validator_for_draft4", r#"type(jsonschema_rs.validator_for({"type": "string"}, draft=4)).__name__"#);

        // Draft validator classes
        run_test!("draft7_validator", r#"type(jsonschema_rs.Draft7Validator({"type": "integer"})).__name__"#);
        run_test!("draft7_mro", r#"[c.__name__ for c in type(jsonschema_rs.Draft7Validator({"type": "integer"})).__mro__]"#);
        run_test!("draft7_has_is_valid", r#"hasattr(jsonschema_rs.Draft7Validator({"type": "integer"}), 'is_valid')"#);
        run_test!("validator_attrs", r#"'is_valid' in dir(jsonschema_rs.validator_for({"type": "string"}))"#);
        run_test!("draft7_is_valid", r#"jsonschema_rs.Draft7Validator({"type": "integer"}).is_valid(42) == True"#);
        run_test!("draft7_invalid", r#"jsonschema_rs.Draft7Validator({"type": "integer"}).is_valid("not_int") == False"#);

        // Schema with constraints
        run_test!("min_length", r#"jsonschema_rs.is_valid({"type": "string", "minLength": 3}, "ab") == False"#);
        run_test!("min_length_valid", r#"jsonschema_rs.is_valid({"type": "string", "minLength": 3}, "abc") == True"#);
        run_test!("max_length", r#"jsonschema_rs.is_valid({"type": "string", "maxLength": 5}, "abcdef") == False"#);
        run_test!("required_fields", r#"jsonschema_rs.is_valid({"type": "object", "required": ["name"]}, {}) == False"#);
        run_test!("required_valid", r#"jsonschema_rs.is_valid({"type": "object", "required": ["name"]}, {"name": "test"}) == True"#);

        // Enum
        run_test!("enum_valid", r#"jsonschema_rs.is_valid({"enum": [1, 2, 3]}, 2) == True"#);
        run_test!("enum_invalid", r#"jsonschema_rs.is_valid({"enum": [1, 2, 3]}, 4) == False"#);

        println!("\nResults: {} passed, {} failed", pass_count, fail_count);
        if fail_count > 0 {
            std::process::exit(1);
        }
    });
}
