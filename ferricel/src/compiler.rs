use cel::common::ast::operators;
use cel::common::ast::Expr;
use cel::common::value::CelVal;
use cel::parser::Parser;
use walrus::{FunctionBuilder, FunctionId, InstrSeqBuilder, ModuleConfig, ValType};

// Embed the runtime WASM at compile time
const RUNTIME_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../target/wasm32-unknown-unknown/release/runtime.wasm"
));

// A struct to hold the handles to your runtime functions
pub struct CompilerEnv {
    // Arithmetic operations
    pub add_func_id: FunctionId,
    pub sub_func_id: FunctionId,
    pub mul_func_id: FunctionId,
    pub div_func_id: FunctionId,
    pub mod_func_id: FunctionId,

    // Comparison operations
    pub eq_func_id: FunctionId,
    pub ne_func_id: FunctionId,
    pub gt_func_id: FunctionId,
    pub lt_func_id: FunctionId,
    pub gte_func_id: FunctionId,
    pub lte_func_id: FunctionId,

    // Logical operations
    pub and_func_id: FunctionId,
    pub or_func_id: FunctionId,
    pub not_func_id: FunctionId,

    // JSON serialization
    pub serialize_value_func_id: FunctionId,

    // JSON deserialization
    pub deserialize_json_func_id: FunctionId,

    // Global variable storage
    pub init_input_func_id: FunctionId,
    pub init_data_func_id: FunctionId,
    pub get_input_func_id: FunctionId,
    pub get_data_func_id: FunctionId,

    // Field access
    pub get_field_func_id: FunctionId,

    // Memory allocation (for field names)
    pub malloc_func_id: FunctionId,

    // Value creation helpers
    pub create_int_func_id: FunctionId,
    pub create_bool_func_id: FunctionId,
}

/// Compile a CEL expression into a WebAssembly module
///
/// Takes a CEL expression string and returns the compiled WASM module as bytes.
/// The resulting module exports a `validate` function with signature (i32, i32) -> i64.
/// The returned i64 encodes a pointer (low 32 bits) and length (high 32 bits) to
/// JSON-serialized result in WASM memory.
pub fn compile_cel_to_wasm(cel_code: &str) -> Result<Vec<u8>, anyhow::Error> {
    // 1. Load the runtime template from embedded bytes
    let mut module = ModuleConfig::new().parse(RUNTIME_BYTES)?;

    // Get the cel_malloc function ID - we'll need it for field names
    let malloc_func_id = module.exports.get_func("cel_malloc")?;

    // 2. Set up the compiler environment
    let env = CompilerEnv {
        // Arithmetic operations
        add_func_id: module.exports.get_func("cel_int_add")?,
        sub_func_id: module.exports.get_func("cel_int_sub")?,
        mul_func_id: module.exports.get_func("cel_int_mul")?,
        div_func_id: module.exports.get_func("cel_int_div")?,
        mod_func_id: module.exports.get_func("cel_int_mod")?,

        // Comparison operations
        eq_func_id: module.exports.get_func("cel_int_eq")?,
        ne_func_id: module.exports.get_func("cel_int_ne")?,
        gt_func_id: module.exports.get_func("cel_int_gt")?,
        lt_func_id: module.exports.get_func("cel_int_lt")?,
        gte_func_id: module.exports.get_func("cel_int_gte")?,
        lte_func_id: module.exports.get_func("cel_int_lte")?,

        // Logical operations
        and_func_id: module.exports.get_func("cel_bool_and")?,
        or_func_id: module.exports.get_func("cel_bool_or")?,
        not_func_id: module.exports.get_func("cel_bool_not")?,

        // JSON serialization
        serialize_value_func_id: module.exports.get_func("cel_serialize_value")?,

        // JSON deserialization
        deserialize_json_func_id: module.exports.get_func("cel_deserialize_json")?,

        // Global variable storage
        init_input_func_id: module.exports.get_func("cel_init_input")?,
        init_data_func_id: module.exports.get_func("cel_init_data")?,
        get_input_func_id: module.exports.get_func("cel_get_input")?,
        get_data_func_id: module.exports.get_func("cel_get_data")?,

        // Field access
        get_field_func_id: module.exports.get_func("cel_get_field")?,

        // Memory allocation
        malloc_func_id,

        // Value creation helpers
        create_int_func_id: module.exports.get_func("cel_create_int")?,
        create_bool_func_id: module.exports.get_func("cel_create_bool")?,
    };

    // 3. Remove the helpers from exports so the Host can't call them directly
    module.exports.remove("cel_int_add")?;
    module.exports.remove("cel_int_sub")?;
    module.exports.remove("cel_int_mul")?;
    module.exports.remove("cel_int_div")?;
    module.exports.remove("cel_int_mod")?;
    module.exports.remove("cel_int_eq")?;
    module.exports.remove("cel_int_ne")?;
    module.exports.remove("cel_int_gt")?;
    module.exports.remove("cel_int_lt")?;
    module.exports.remove("cel_int_gte")?;
    module.exports.remove("cel_int_lte")?;
    module.exports.remove("cel_bool_and")?;
    module.exports.remove("cel_bool_or")?;
    module.exports.remove("cel_bool_not")?;
    module.exports.remove("cel_serialize_value")?;
    module.exports.remove("cel_deserialize_json")?;
    module.exports.remove("cel_init_input")?;
    module.exports.remove("cel_init_data")?;
    module.exports.remove("cel_get_input")?;
    module.exports.remove("cel_get_data")?;
    module.exports.remove("cel_get_field")?;
    module.exports.remove("cel_create_int")?;
    module.exports.remove("cel_create_bool")?;

    // 4. Parse the CEL expression
    let root_ast = Parser::default()
        .parse(cel_code)
        .map_err(|e| anyhow::anyhow!("Parse error: {:?}", e))?;

    // 5. Build the 'validate' function (i64, i64) -> i64
    // First i64: encoded (ptr, len) for input JSON (0 if no input)
    // Second i64: encoded (ptr, len) for data JSON (0 if no data)
    let mut validate_func = FunctionBuilder::new(
        &mut module.types,
        &[ValType::I64, ValType::I64],
        &[ValType::I64],
    );
    let input_encoded_arg = module.locals.add(ValType::I64);
    let data_encoded_arg = module.locals.add(ValType::I64);

    let mut body = validate_func.func_body();

    // 6. Initialize global variables (input and data)
    // Deserialize input (first parameter) and store in global
    body.local_get(input_encoded_arg)
        .call(env.deserialize_json_func_id) // Returns *mut CelValue
        .call(env.init_input_func_id); // Store in INPUT_VALUE global

    // Deserialize data (second parameter) and store in global
    body.local_get(data_encoded_arg)
        .call(env.deserialize_json_func_id) // Returns *mut CelValue
        .call(env.init_data_func_id); // Store in DATA_VALUE global

    // 7. Walk the AST and compile to WASM instructions
    // This leaves a *mut CelValue on the stack
    compile_expr(&root_ast.expr, &mut body, &env, &mut module)?;

    // 8. Serialize the result to JSON
    // The stack has a *mut CelValue, serialize it directly
    body.call(env.serialize_value_func_id);

    // 9. Finish the function definition
    let validate_id =
        validate_func.finish(vec![input_encoded_arg, data_encoded_arg], &mut module.funcs);

    // 10. Export the 'validate' function for the Host
    module.exports.add("validate", validate_id);

    // 11. Emit the module as bytes
    let wasm_bytes = module.emit_wasm();

    Ok(wasm_bytes)
}

// We pass the inner `Expr` enum to this recursive function
// This always leaves a *mut CelValue (i32) on the stack
pub fn compile_expr(
    expr: &Expr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match expr {
        // 1. Literal Values
        Expr::Literal(literal) => {
            match literal {
                CelVal::Int(value) => {
                    // Create a CelValue::Int pointer
                    body.i64_const(*value);
                    body.call(env.create_int_func_id);
                }
                CelVal::Boolean(b) => {
                    // Create a CelValue::Bool pointer
                    body.i64_const(if *b { 1 } else { 0 });
                    body.call(env.create_bool_func_id);
                }
                // String literals require memory allocation, which we haven't built yet!
                _ => anyhow::bail!("Unsupported literal: {:?}", literal),
            }
        }

        // 2. Function Calls (including operators)
        // In CEL, operators like +, ==, > are represented as Call expressions
        // with special function names like "_+_", "_==_", "_>_"
        Expr::Call(call_expr) => {
            match call_expr.func_name.as_str() {
                // Arithmetic operators
                operators::ADD => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Add operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, module)?;
                    body.call(env.add_func_id);
                }
                operators::SUBSTRACT => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Subtract operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, module)?;
                    body.call(env.sub_func_id);
                }
                operators::MULTIPLY => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Multiply operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, module)?;
                    body.call(env.mul_func_id);
                }
                operators::DIVIDE => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Divide operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, module)?;
                    body.call(env.div_func_id);
                }
                operators::MODULO => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Modulo operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, module)?;
                    body.call(env.mod_func_id);
                }

                // Comparison operators
                operators::EQUALS => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Equals operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, module)?;
                    body.call(env.eq_func_id);
                }
                operators::NOT_EQUALS => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Not equals operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, module)?;
                    body.call(env.ne_func_id);
                }
                operators::GREATER => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Greater than operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, module)?;
                    body.call(env.gt_func_id);
                }
                operators::LESS => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Less than operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, module)?;
                    body.call(env.lt_func_id);
                }
                operators::GREATER_EQUALS => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Greater or equal operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, module)?;
                    body.call(env.gte_func_id);
                }
                operators::LESS_EQUALS => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Less or equal operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, module)?;
                    body.call(env.lte_func_id);
                }

                // Logical operators
                operators::LOGICAL_AND => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Logical AND operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, module)?;
                    body.call(env.and_func_id);
                }
                operators::LOGICAL_OR => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Logical OR operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, module)?;
                    body.call(env.or_func_id);
                }
                operators::LOGICAL_NOT => {
                    if call_expr.args.len() != 1 {
                        anyhow::bail!("Logical NOT operator expects 1 argument");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, module)?;
                    body.call(env.not_func_id);
                }

                _ => anyhow::bail!("Unsupported function call: {}", call_expr.func_name),
            }
        }

        // 3. Identifiers (variables)
        Expr::Ident(name) => {
            // Support 'input' and 'data' variables
            match name.as_str() {
                "input" => {
                    // Get the input variable from global storage
                    // Returns *mut CelValue
                    body.call(env.get_input_func_id);
                }
                "data" => {
                    // Get the data variable from global storage
                    // Returns *mut CelValue
                    body.call(env.get_data_func_id);
                }
                _ => {
                    anyhow::bail!(
                        "Unknown variable: {}. Only 'input' and 'data' are supported.",
                        name
                    );
                }
            }
        }

        // 4. Field selection (e.g., object.field, input.user.name)
        Expr::Select(select_expr) => {
            // Recursively compile the operand as a pointer (e.g., `input` or `input.user`)
            // This will leave a *mut CelValue (i32) on the stack
            compile_expr(&select_expr.operand.expr, body, env, module)?;

            // Now we need to get the field from the object
            // The operand (object pointer) is on the stack
            // We need to pass: (obj_ptr, field_name_ptr, field_name_len)

            let field_name = &select_expr.field;
            let field_bytes = field_name.as_bytes();
            let field_len = field_bytes.len() as i32;

            // Create a local to store the field name pointer
            let field_ptr_local = module.locals.add(ValType::I32);

            // Allocate memory for the field name
            body.i32_const(field_len)
                .call(env.malloc_func_id) // Returns field_name_ptr
                .local_tee(field_ptr_local); // Store in local and keep on stack

            // Stack is now: [obj_ptr, field_name_ptr]

            // We need to write the field name bytes to the allocated memory
            // Get memory reference
            let memory_id = module
                .memories
                .iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("No memory found"))?
                .id();

            // Write each byte of the field name
            for (offset, &byte) in field_bytes.iter().enumerate() {
                // Load field_name_ptr
                body.local_get(field_ptr_local);
                // Load byte value
                body.i32_const(byte as i32);
                // Store byte at offset
                body.store(
                    memory_id,
                    walrus::ir::StoreKind::I32_8 { atomic: false },
                    walrus::ir::MemArg {
                        align: 1,
                        offset: offset as u32,
                    },
                );
            }

            // Now call cel_get_field(obj_ptr, field_ptr, field_len)
            // Stack currently has: [obj_ptr, field_name_ptr]
            // We need: [obj_ptr, field_name_ptr, field_len]
            body.i32_const(field_len);

            // Call cel_get_field - returns *mut CelValue
            body.call(env.get_field_func_id);
        }

        _ => anyhow::bail!("Unsupported expression type: {:?}", expr),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime;
    use rstest::rstest;

    /// Test helper: compile CEL expression and execute it, returning the result
    fn compile_and_execute(cel_expr: &str) -> Result<i64, anyhow::Error> {
        let wasm_bytes = compile_cel_to_wasm(cel_expr)?;
        let json_result = runtime::execute_wasm_with_vars(&wasm_bytes, None, None)?;

        // Parse JSON to extract the numeric value
        // The JSON will be either an integer (e.g., "42") or boolean (e.g., "true"/"false")
        let value: serde_json::Value = serde_json::from_str(&json_result)?;

        match value {
            serde_json::Value::Number(n) => n
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("Expected i64, got: {}", n)),
            serde_json::Value::Bool(b) => Ok(if b { 1 } else { 0 }),
            _ => anyhow::bail!("Unexpected JSON value type: {}", value),
        }
    }

    /// Test helper: compile CEL expression with variables and execute it
    fn compile_and_execute_with_vars(
        cel_expr: &str,
        input_json: Option<&str>,
        data_json: Option<&str>,
    ) -> Result<i64, anyhow::Error> {
        let wasm_bytes = compile_cel_to_wasm(cel_expr)?;
        let json_result = runtime::execute_wasm_with_vars(&wasm_bytes, input_json, data_json)?;

        // Parse JSON to extract the numeric value
        let value: serde_json::Value = serde_json::from_str(&json_result)?;

        match value {
            serde_json::Value::Number(n) => n
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("Expected i64, got: {}", n)),
            serde_json::Value::Bool(b) => Ok(if b { 1 } else { 0 }),
            _ => anyhow::bail!("Unexpected JSON value type: {}", value),
        }
    }

    #[rstest]
    #[case("42", 42)]
    #[case("0", 0)]
    #[case("1", 1)]
    #[case("-5", -5)]
    #[case("9999", 9999)]
    fn test_literal_integers(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    #[rstest]
    #[case("1 + 1", 2)]
    #[case("10 + 20", 30)]
    #[case("5 + 7", 12)]
    #[case("100 + 200", 300)]
    #[case("0 + 0", 0)]
    #[case("-5 + 10", 5)]
    #[case("10 + -5", 5)]
    fn test_simple_addition(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    #[rstest]
    #[case("1 + 2 + 3", 6)]
    #[case("10 + 20 + 30", 60)]
    #[case("1 + 2 + 3 + 4 + 5", 15)]
    #[case("100 + 200 + 300", 600)]
    #[case("1 + 1 + 1 + 1 + 1 + 1", 6)]
    #[case("10 + 20 + 30 + 40 + 50", 150)]
    fn test_chained_addition(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    #[rstest]
    #[case("(10 + 20)", 30)]
    #[case("((5 + 5))", 10)]
    #[case("(1 + 2) + 3", 6)]
    #[case("1 + (2 + 3)", 6)]
    #[case("(1 + 2) + (3 + 4)", 10)]
    fn test_parenthesized_expressions(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    #[rstest]
    #[case("0 + 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9", 45)]
    #[case("100 + 200 + 300 + 400 + 500", 1500)]
    fn test_large_expressions(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    #[test]
    fn test_compile_cel_to_wasm_returns_valid_bytes() {
        let wasm_bytes = compile_cel_to_wasm("42").expect("Failed to compile");
        assert!(!wasm_bytes.is_empty(), "WASM bytes should not be empty");

        // WASM files start with magic number: 0x00 0x61 0x73 0x6D (\\0asm)
        assert_eq!(
            &wasm_bytes[0..4],
            &[0x00, 0x61, 0x73, 0x6D],
            "Should have WASM magic number"
        );
    }

    #[test]
    fn test_invalid_cel_expression() {
        let result = compile_cel_to_wasm("1 + + 2");
        assert!(
            result.is_err(),
            "Invalid CEL expression should return error"
        );
    }

    #[test]
    fn test_unsupported_operation() {
        let result = compile_cel_to_wasm("my_var");
        assert!(
            result.is_err(),
            "Variable access should not be supported yet"
        );
    }

    // ===== Subtraction Tests =====
    #[rstest]
    #[case("10 - 5", 5)]
    #[case("100 - 50", 50)]
    #[case("5 - 10", -5)]
    #[case("0 - 5", -5)]
    #[case("10 - 0", 10)]
    #[case("-5 - 10", -15)]
    #[case("10 - -5", 15)]
    fn test_subtraction(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    // ===== Multiplication Tests =====
    #[rstest]
    #[case("2 * 3", 6)]
    #[case("5 * 5", 25)]
    #[case("10 * 10", 100)]
    #[case("0 * 100", 0)]
    #[case("100 * 0", 0)]
    #[case("-5 * 3", -15)]
    #[case("5 * -3", -15)]
    #[case("-5 * -3", 15)]
    fn test_multiplication(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    // ===== Division Tests =====
    #[rstest]
    #[case("10 / 2", 5)]
    #[case("100 / 10", 10)]
    #[case("7 / 2", 3)] // Integer division
    #[case("0 / 5", 0)]
    #[case("-10 / 2", -5)]
    #[case("10 / -2", -5)]
    #[case("-10 / -2", 5)]
    fn test_division(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    #[test]
    fn test_division_by_zero() {
        let result = compile_and_execute("10 / 0");
        assert!(
            result.is_err(),
            "Division by zero should produce an error, got: {:?}",
            result
        );
    }

    #[test]
    fn test_modulo_by_zero() {
        let result = compile_and_execute("10 % 0");
        assert!(
            result.is_err(),
            "Modulo by zero should produce an error, got: {:?}",
            result
        );
    }

    // ===== Modulo Tests =====
    #[rstest]
    #[case("10 % 3", 1)]
    #[case("100 % 7", 2)]
    #[case("5 % 5", 0)]
    #[case("3 % 10", 3)]
    #[case("0 % 5", 0)]
    fn test_modulo(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    // ===== Mixed Arithmetic Tests =====
    #[rstest]
    #[case("2 + 3 * 4", 14)] // CEL respects precedence: 3*4 first, then +2
    #[case("10 - 2 * 3", 4)] // 2*3 first, then 10-6
    #[case("20 / 4 + 3", 8)] // 20/4 first, then +3
    #[case("(2 + 3) * 4", 20)] // Parentheses override precedence
    #[case("10 * 2 + 5 * 3", 35)] // 10*2 + 5*3 = 20 + 15
    fn test_mixed_arithmetic(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    // ===== Comparison Tests =====
    #[rstest]
    #[case("5 == 5", 1)]
    #[case("5 == 10", 0)]
    #[case("10 == 5", 0)]
    #[case("0 == 0", 1)]
    #[case("-5 == -5", 1)]
    fn test_equality(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    #[rstest]
    #[case("5 != 5", 0)]
    #[case("5 != 10", 1)]
    #[case("10 != 5", 1)]
    #[case("0 != 0", 0)]
    fn test_not_equals(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    #[rstest]
    #[case("10 > 5", 1)]
    #[case("5 > 10", 0)]
    #[case("5 > 5", 0)]
    #[case("0 > -5", 1)]
    #[case("-5 > 0", 0)]
    fn test_greater_than(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    #[rstest]
    #[case("5 < 10", 1)]
    #[case("10 < 5", 0)]
    #[case("5 < 5", 0)]
    #[case("-5 < 0", 1)]
    #[case("0 < -5", 0)]
    fn test_less_than(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    #[rstest]
    #[case("10 >= 5", 1)]
    #[case("5 >= 10", 0)]
    #[case("5 >= 5", 1)]
    #[case("0 >= -5", 1)]
    fn test_greater_or_equal(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    #[rstest]
    #[case("5 <= 10", 1)]
    #[case("10 <= 5", 0)]
    #[case("5 <= 5", 1)]
    #[case("-5 <= 0", 1)]
    fn test_less_or_equal(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    // ===== Logical Operator Tests =====
    #[rstest]
    #[case("true && true", 1)]
    #[case("true && false", 0)]
    #[case("false && true", 0)]
    #[case("false && false", 0)]
    fn test_logical_and(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    #[rstest]
    #[case("true || true", 1)]
    #[case("true || false", 1)]
    #[case("false || true", 1)]
    #[case("false || false", 0)]
    fn test_logical_or(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    #[rstest]
    #[case("!true", 0)]
    #[case("!false", 1)]
    fn test_logical_not(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    // ===== Combined Logic and Comparison Tests =====
    #[rstest]
    #[case("5 > 3 && 10 > 7", 1)]
    #[case("5 > 10 && 10 > 7", 0)]
    #[case("5 > 3 || 10 < 7", 1)]
    #[case("5 < 3 || 10 < 7", 0)]
    #[case("!(5 > 10)", 1)]
    #[case("!(5 > 3)", 0)]
    #[case("5 == 5 && 10 == 10", 1)]
    #[case("5 != 10 || 3 == 3", 1)]
    fn test_combined_logic_and_comparison(#[case] expr: &str, #[case] expected: i64) {
        let result = compile_and_execute(expr).expect("Failed to compile and execute");
        assert_eq!(
            result, expected,
            "Expression '{}' should evaluate to {}",
            expr, expected
        );
    }

    #[test]
    fn test_integer_overflow_addition() {
        let expr = "9223372036854775807 + 1"; // i64::MAX + 1
        let result = compile_and_execute(expr);
        assert!(
            result.is_err(),
            "Addition overflow should produce an error, got: {:?}",
            result
        );
    }

    #[test]
    fn test_integer_overflow_subtraction() {
        let expr = "-9223372036854775808 - 1"; // i64::MIN - 1
        let result = compile_and_execute(expr);
        assert!(
            result.is_err(),
            "Subtraction overflow should produce an error, got: {:?}",
            result
        );
    }

    #[test]
    fn test_integer_overflow_multiplication() {
        let expr = "9223372036854775807 * 2"; // i64::MAX * 2
        let result = compile_and_execute(expr);
        assert!(
            result.is_err(),
            "Multiplication overflow should produce an error, got: {:?}",
            result
        );
    }

    #[test]
    fn test_special_division_overflow() {
        let expr = "-9223372036854775808 / -1"; // i64::MIN / -1
        let result = compile_and_execute(expr);
        assert!(
            result.is_err(),
            "Special case division overflow (i64::MIN / -1) should produce an error, got: {:?}",
            result
        );
    }

    #[test]
    fn test_special_modulo_overflow() {
        let expr = "-9223372036854775808 % -1"; // i64::MIN % -1
        let result = compile_and_execute(expr);
        assert!(
            result.is_err(),
            "Special case modulo overflow (i64::MIN % -1) should produce an error, got: {:?}",
            result
        );
    }

    #[test]
    fn test_safe_arithmetic_at_boundaries() {
        // These operations should work without overflow
        let result = compile_and_execute("9223372036854775807 - 1")
            .expect("i64::MAX - 1 should not overflow");
        assert_eq!(result, 9223372036854775806);

        let result = compile_and_execute("-9223372036854775808 + 1")
            .expect("i64::MIN + 1 should not overflow");
        assert_eq!(result, -9223372036854775807);

        let result = compile_and_execute("4611686018427387903 * 2")
            .expect("(i64::MAX / 2) * 2 should not overflow");
        assert_eq!(result, 9223372036854775806);
    }

    #[test]
    fn test_negative_overflow_addition() {
        let expr = "-9223372036854775808 + -1"; // i64::MIN + -1
        let result = compile_and_execute(expr);
        assert!(
            result.is_err(),
            "Addition resulting in negative overflow should produce an error, got: {:?}",
            result
        );
    }

    #[test]
    fn test_positive_overflow_subtraction() {
        let expr = "9223372036854775807 - -1"; // i64::MAX - (-1)
        let result = compile_and_execute(expr);
        assert!(
            result.is_err(),
            "Subtraction resulting in positive overflow should produce an error, got: {:?}",
            result
        );
    }

    #[test]
    fn test_json_output_integer() {
        // Test that integers are serialized as raw JSON numbers
        let wasm_bytes = compile_cel_to_wasm("42").expect("Failed to compile");
        let json_result =
            runtime::execute_wasm_with_vars(&wasm_bytes, None, None).expect("Failed to execute");
        assert_eq!(
            json_result, "42",
            "Integer should be serialized as raw JSON number"
        );
    }

    #[test]
    fn test_json_output_boolean_true() {
        // Test that true is serialized as raw JSON boolean
        let wasm_bytes = compile_cel_to_wasm("5 > 3").expect("Failed to compile");
        let json_result =
            runtime::execute_wasm_with_vars(&wasm_bytes, None, None).expect("Failed to execute");
        assert_eq!(
            json_result, "true",
            "Boolean true should be serialized as 'true'"
        );
    }

    #[test]
    fn test_json_output_boolean_false() {
        // Test that false is serialized as raw JSON boolean
        let wasm_bytes = compile_cel_to_wasm("5 < 3").expect("Failed to compile");
        let json_result =
            runtime::execute_wasm_with_vars(&wasm_bytes, None, None).expect("Failed to execute");
        assert_eq!(
            json_result, "false",
            "Boolean false should be serialized as 'false'"
        );
    }

    #[test]
    fn test_json_output_negative_integer() {
        // Test that negative integers are properly serialized
        let wasm_bytes = compile_cel_to_wasm("-123").expect("Failed to compile");
        let json_result =
            runtime::execute_wasm_with_vars(&wasm_bytes, None, None).expect("Failed to execute");
        assert_eq!(
            json_result, "-123",
            "Negative integer should be serialized correctly"
        );
    }

    #[test]
    fn test_json_output_arithmetic_result() {
        // Test that arithmetic results are serialized correctly
        let wasm_bytes = compile_cel_to_wasm("10 + 20 * 2").expect("Failed to compile");
        let json_result =
            runtime::execute_wasm_with_vars(&wasm_bytes, None, None).expect("Failed to execute");
        assert_eq!(
            json_result, "50",
            "Arithmetic result should be serialized correctly"
        );
    }

    // ========================================
    // Variable Access Tests (PR #4)
    // ========================================

    #[test]
    fn test_input_variable_positive() {
        // Test accessing input variable with a positive integer
        let result =
            compile_and_execute_with_vars("input", Some("42"), None).expect("Failed to execute");
        assert_eq!(result, 42, "input should return 42");
    }

    #[test]
    fn test_input_variable_negative() {
        // Test accessing input variable with a negative integer
        let result =
            compile_and_execute_with_vars("input", Some("-10"), None).expect("Failed to execute");
        assert_eq!(result, -10, "input should return -10");
    }

    #[test]
    fn test_input_variable_zero() {
        // Test accessing input variable with zero
        let result =
            compile_and_execute_with_vars("input", Some("0"), None).expect("Failed to execute");
        assert_eq!(result, 0, "input should return 0");
    }

    #[test]
    fn test_data_variable_positive() {
        // Test accessing data variable with a positive integer
        let result =
            compile_and_execute_with_vars("data", None, Some("100")).expect("Failed to execute");
        assert_eq!(result, 100, "data should return 100");
    }

    #[test]
    fn test_data_variable_negative() {
        // Test accessing data variable with a negative integer
        let result =
            compile_and_execute_with_vars("data", None, Some("-50")).expect("Failed to execute");
        assert_eq!(result, -50, "data should return -50");
    }

    #[test]
    fn test_input_and_data_addition() {
        // Test using both input and data in an expression
        let result = compile_and_execute_with_vars("input + data", Some("10"), Some("20"))
            .expect("Failed to execute");
        assert_eq!(result, 30, "input + data should return 30");
    }

    #[test]
    fn test_input_and_data_multiplication() {
        // Test multiplication with input and data
        let result = compile_and_execute_with_vars("input * data", Some("5"), Some("7"))
            .expect("Failed to execute");
        assert_eq!(result, 35, "input * data should return 35");
    }

    #[test]
    fn test_input_in_complex_expression() {
        // Test input in a more complex expression
        let result = compile_and_execute_with_vars("input * 2 + 10", Some("5"), None)
            .expect("Failed to execute");
        assert_eq!(result, 20, "input * 2 + 10 should return 20");
    }

    #[test]
    fn test_data_in_complex_expression() {
        // Test data in a more complex expression
        let result = compile_and_execute_with_vars("(data - 5) * 3", None, Some("10"))
            .expect("Failed to execute");
        assert_eq!(result, 15, "(data - 5) * 3 should return 15");
    }

    #[test]
    fn test_input_variable_i64_max() {
        // Test with i64::MAX
        let max = i64::MAX;
        let input_json = format!("{}", max);
        let result = compile_and_execute_with_vars("input", Some(&input_json), None)
            .expect("Failed to execute");
        assert_eq!(result, max, "input should return i64::MAX");
    }

    #[test]
    fn test_input_variable_i64_min() {
        // Test with i64::MIN
        let min = i64::MIN;
        let input_json = format!("{}", min);
        let result = compile_and_execute_with_vars("input", Some(&input_json), None)
            .expect("Failed to execute");
        assert_eq!(result, min, "input should return i64::MIN");
    }

    // ========================================
    // Field Access Tests
    // ========================================

    #[test]
    fn test_simple_field_access() {
        // Test accessing a field from input object
        let input_json = r#"{"age": 42}"#;
        let result = compile_and_execute_with_vars("input.age", Some(input_json), None)
            .expect("Failed to execute");
        assert_eq!(result, 42, "input.age should return 42");
    }

    #[test]
    fn test_nested_field_access() {
        // Test accessing nested fields
        let input_json = r#"{"user": {"age": 30}}"#;
        let result = compile_and_execute_with_vars("input.user.age", Some(input_json), None)
            .expect("Failed to execute");
        assert_eq!(result, 30, "input.user.age should return 30");
    }

    #[test]
    fn test_field_access_with_data() {
        // Test field access on data variable
        let data_json = r#"{"count": 100}"#;
        let result = compile_and_execute_with_vars("data.count", None, Some(data_json))
            .expect("Failed to execute");
        assert_eq!(result, 100, "data.count should return 100");
    }

    #[test]
    fn test_field_access_in_expression() {
        // Test using field access in arithmetic
        let input_json = r#"{"x": 10}"#;
        let result = compile_and_execute_with_vars("input.x * 2 + 5", Some(input_json), None)
            .expect("Failed to execute");
        assert_eq!(result, 25, "input.x * 2 + 5 should return 25");
    }

    #[test]
    fn test_multiple_field_access() {
        // Test accessing fields from both input and data
        let input_json = r#"{"a": 10}"#;
        let data_json = r#"{"b": 20}"#;
        let result =
            compile_and_execute_with_vars("input.a + data.b", Some(input_json), Some(data_json))
                .expect("Failed to execute");
        assert_eq!(result, 30, "input.a + data.b should return 30");
    }

    #[test]
    fn test_deeply_nested_field_access() {
        // Test accessing deeply nested fields
        let input_json = r#"{"level1": {"level2": {"level3": {"value": 99}}}}"#;
        let result = compile_and_execute_with_vars(
            "input.level1.level2.level3.value",
            Some(input_json),
            None,
        )
        .expect("Failed to execute");
        assert_eq!(result, 99, "deeply nested field should return 99");
    }
}
