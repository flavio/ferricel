use cel::common::ast::Expr;
use cel::common::ast::operators;
use cel::common::value::CelVal;
use cel::parser::Parser;
use std::collections::HashMap;
use walrus::{FunctionBuilder, FunctionId, InstrSeqBuilder, LocalId, ModuleConfig, ValType};

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
    pub negate_func_id: FunctionId,

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
    pub not_strictly_false_func_id: FunctionId,
    pub conditional_func_id: FunctionId,

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
    pub has_field_func_id: FunctionId,

    // Array operations
    pub array_len_func_id: FunctionId,
    pub array_get_func_id: FunctionId,
    pub create_array_func_id: FunctionId,
    pub array_push_func_id: FunctionId,

    // Map operations
    pub create_map_func_id: FunctionId,
    pub map_insert_func_id: FunctionId,

    // Memory allocation (for field names)
    pub malloc_func_id: FunctionId,

    // Value creation helpers
    pub create_int_func_id: FunctionId,
    pub create_uint_func_id: FunctionId,
    pub create_bool_func_id: FunctionId,
    pub create_double_func_id: FunctionId,
    pub create_string_func_id: FunctionId,
    pub create_bytes_func_id: FunctionId,
    pub create_null_func_id: FunctionId,

    // String operations
    pub size_func_id: FunctionId,
    pub string_starts_with_func_id: FunctionId,
    pub string_ends_with_func_id: FunctionId,
    pub string_contains_func_id: FunctionId,
    pub string_matches_func_id: FunctionId,

    // Membership testing
    pub in_func_id: FunctionId,

    // Index operator
    pub index_func_id: FunctionId,

    // Value conversion helpers
    pub value_to_bool_func_id: FunctionId,

    // Temporal operations
    pub timestamp_func_id: FunctionId,
    pub duration_func_id: FunctionId,

    // Value conversion helpers
    pub string_func_id: FunctionId,
    pub int_func_id: FunctionId,
    pub uint_func_id: FunctionId,
    pub double_func_id: FunctionId,
    pub bytes_func_id: FunctionId,
}

/// Compilation context that holds state during expression compilation
/// This includes local variable bindings for comprehensions and other scoped contexts
#[derive(Default)]
pub struct CompilerContext {
    /// Maps variable names to their local IDs in the WASM function
    /// Used for iteration variables in comprehensions (e.g., "x" in [1,2,3].all(x, x > 0))
    pub local_vars: HashMap<String, LocalId>,
}

impl CompilerContext {
    /// Create a child context with an additional local variable binding
    /// This is used when entering a new scope (e.g., comprehension)
    pub fn with_local(&self, name: String, local_id: LocalId) -> Self {
        let mut local_vars = self.local_vars.clone();
        local_vars.insert(name, local_id);
        Self { local_vars }
    }
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
        add_func_id: module.exports.get_func("cel_value_add")?,
        sub_func_id: module.exports.get_func("cel_value_sub")?,
        mul_func_id: module.exports.get_func("cel_value_mul")?,
        div_func_id: module.exports.get_func("cel_value_div")?,
        mod_func_id: module.exports.get_func("cel_value_mod")?,
        negate_func_id: module.exports.get_func("cel_value_negate")?,

        // Comparison operations
        eq_func_id: module.exports.get_func("cel_value_eq")?,
        ne_func_id: module.exports.get_func("cel_value_ne")?,
        gt_func_id: module.exports.get_func("cel_value_gt")?,
        lt_func_id: module.exports.get_func("cel_value_lt")?,
        gte_func_id: module.exports.get_func("cel_value_gte")?,
        lte_func_id: module.exports.get_func("cel_value_lte")?,

        // Logical operations
        and_func_id: module.exports.get_func("cel_bool_and")?,
        or_func_id: module.exports.get_func("cel_bool_or")?,
        not_func_id: module.exports.get_func("cel_bool_not")?,
        not_strictly_false_func_id: module.exports.get_func("cel_not_strictly_false")?,
        conditional_func_id: module.exports.get_func("cel_conditional")?,

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
        has_field_func_id: module.exports.get_func("cel_has_field")?,

        // Array operations
        array_len_func_id: module.exports.get_func("cel_array_len")?,
        array_get_func_id: module.exports.get_func("cel_array_get")?,
        create_array_func_id: module.exports.get_func("cel_create_array")?,
        array_push_func_id: module.exports.get_func("cel_array_push")?,

        // Map operations
        create_map_func_id: module.exports.get_func("cel_create_map")?,
        map_insert_func_id: module.exports.get_func("cel_map_insert")?,

        // Memory allocation
        malloc_func_id,

        // Value creation helpers
        create_int_func_id: module.exports.get_func("cel_create_int")?,
        create_uint_func_id: module.exports.get_func("cel_create_uint")?,
        create_bool_func_id: module.exports.get_func("cel_create_bool")?,
        create_double_func_id: module.exports.get_func("cel_create_double")?,
        create_string_func_id: module.exports.get_func("cel_create_string")?,
        create_bytes_func_id: module.exports.get_func("cel_create_bytes")?,
        create_null_func_id: module.exports.get_func("cel_create_null")?,

        // String operations
        size_func_id: module.exports.get_func("cel_value_size")?,
        string_starts_with_func_id: module.exports.get_func("cel_string_starts_with")?,
        string_ends_with_func_id: module.exports.get_func("cel_string_ends_with")?,
        string_contains_func_id: module.exports.get_func("cel_string_contains")?,
        string_matches_func_id: module.exports.get_func("cel_string_matches")?,

        // Membership testing
        in_func_id: module.exports.get_func("cel_value_in")?,

        // Index operator
        index_func_id: module.exports.get_func("cel_value_index")?,

        // Value conversion helpers
        value_to_bool_func_id: module.exports.get_func("cel_value_to_bool")?,

        // Temporal operations
        timestamp_func_id: module.exports.get_func("cel_timestamp")?,
        duration_func_id: module.exports.get_func("cel_duration")?,
        string_func_id: module.exports.get_func("cel_string")?,
        int_func_id: module.exports.get_func("cel_int")?,
        uint_func_id: module.exports.get_func("cel_uint")?,
        double_func_id: module.exports.get_func("cel_double")?,
        bytes_func_id: module.exports.get_func("cel_bytes")?,
    };

    // 3. Remove the helpers from exports so the Host can't call them directly
    module.exports.remove("cel_value_add")?;
    module.exports.remove("cel_value_sub")?;
    module.exports.remove("cel_value_mul")?;
    module.exports.remove("cel_value_div")?;
    module.exports.remove("cel_value_mod")?;
    module.exports.remove("cel_value_eq")?;
    module.exports.remove("cel_value_ne")?;
    module.exports.remove("cel_value_gt")?;
    module.exports.remove("cel_value_lt")?;
    module.exports.remove("cel_value_gte")?;
    module.exports.remove("cel_value_lte")?;
    // Keep type-specific functions hidden (used internally by polymorphic functions)
    module.exports.remove("cel_int_sub")?;
    module.exports.remove("cel_int_mul")?;
    module.exports.remove("cel_int_div")?;
    module.exports.remove("cel_int_mod")?;
    module.exports.remove("cel_uint_add")?;
    module.exports.remove("cel_uint_sub")?;
    module.exports.remove("cel_uint_mul")?;
    module.exports.remove("cel_uint_div")?;
    module.exports.remove("cel_uint_mod")?;
    module.exports.remove("cel_double_add")?;
    module.exports.remove("cel_double_sub")?;
    module.exports.remove("cel_double_mul")?;
    module.exports.remove("cel_double_div")?;
    module.exports.remove("cel_int_eq")?;
    module.exports.remove("cel_int_ne")?;
    module.exports.remove("cel_int_gt")?;
    module.exports.remove("cel_int_lt")?;
    module.exports.remove("cel_int_gte")?;
    module.exports.remove("cel_int_lte")?;
    module.exports.remove("cel_uint_eq")?;
    module.exports.remove("cel_uint_ne")?;
    module.exports.remove("cel_uint_gt")?;
    module.exports.remove("cel_uint_lt")?;
    module.exports.remove("cel_uint_gte")?;
    module.exports.remove("cel_uint_lte")?;
    module.exports.remove("cel_double_eq")?;
    module.exports.remove("cel_double_ne")?;
    module.exports.remove("cel_double_gt")?;
    module.exports.remove("cel_double_lt")?;
    module.exports.remove("cel_double_gte")?;
    module.exports.remove("cel_double_lte")?;
    module.exports.remove("cel_bool_and")?;
    module.exports.remove("cel_bool_or")?;
    module.exports.remove("cel_bool_not")?;
    module.exports.remove("cel_not_strictly_false")?;
    module.exports.remove("cel_conditional")?;
    module.exports.remove("cel_serialize_value")?;
    module.exports.remove("cel_deserialize_json")?;
    module.exports.remove("cel_init_input")?;
    module.exports.remove("cel_init_data")?;
    module.exports.remove("cel_get_input")?;
    module.exports.remove("cel_get_data")?;
    module.exports.remove("cel_get_field")?;
    module.exports.remove("cel_has_field")?;
    module.exports.remove("cel_array_len")?;
    module.exports.remove("cel_array_get")?;
    module.exports.remove("cel_create_array")?;
    module.exports.remove("cel_array_push")?;
    module.exports.remove("cel_create_map")?;
    module.exports.remove("cel_map_insert")?;
    module.exports.remove("cel_create_int")?;
    module.exports.remove("cel_create_uint")?;
    module.exports.remove("cel_create_bool")?;
    module.exports.remove("cel_create_double")?;
    module.exports.remove("cel_value_to_bool")?;
    module.exports.remove("cel_value_to_i64")?;
    module.exports.remove("cel_value_to_u64")?;
    module.exports.remove("cel_int")?;
    module.exports.remove("cel_uint")?;
    module.exports.remove("cel_double")?;
    module.exports.remove("cel_value_in")?;
    module.exports.remove("cel_value_index")?;

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
    let ctx = CompilerContext::default();
    compile_expr(&root_ast.expr, &mut body, &env, &ctx, &mut module)?;

    // 8. Serialize the result to JSON
    // The stack has a *mut CelValue, serialize it directly
    body.call(env.serialize_value_func_id);

    // 9. Finish the function definition
    let validate_id =
        validate_func.finish(vec![input_encoded_arg, data_encoded_arg], &mut module.funcs);

    // 10. Export the 'validate' function for the Host
    module.exports.add("validate", validate_id);

    // 11. Run garbage collection to remove unreferenced items (dead code elimination)
    walrus::passes::gc::run(&mut module);

    // 12. Emit the module as bytes
    let wasm_bytes = module.emit_wasm();

    Ok(wasm_bytes)
}

// We pass the inner `Expr` enum to this recursive function
// This always leaves a *mut CelValue (i32) on the stack
pub fn compile_expr(
    expr: &Expr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
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
                CelVal::UInt(value) => {
                    // Create a CelValue::UInt pointer
                    // Note: WASM only has i64, so we pass u64 as i64
                    body.i64_const(*value as i64);
                    body.call(env.create_uint_func_id);
                }
                CelVal::Boolean(b) => {
                    // Create a CelValue::Bool pointer
                    body.i64_const(if *b { 1 } else { 0 });
                    body.call(env.create_bool_func_id);
                }
                CelVal::Double(d) => {
                    // Create a CelValue::Double pointer
                    body.f64_const(*d);
                    body.call(env.create_double_func_id);
                }
                CelVal::String(s) => {
                    // String literals require memory allocation
                    let string_bytes = s.as_bytes();
                    let string_len = string_bytes.len() as i32;

                    // Create a local to store the string data pointer
                    let data_ptr_local = module.locals.add(ValType::I32);

                    // Allocate memory for the string data
                    body.i32_const(string_len)
                        .call(env.malloc_func_id) // Returns data_ptr
                        .local_set(data_ptr_local); // Store in local and pop from stack

                    // Get memory reference
                    let memory_id = module
                        .memories
                        .iter()
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("No memory found"))?
                        .id();

                    // Write each byte of the string to the allocated memory
                    for (offset, &byte) in string_bytes.iter().enumerate() {
                        // Load data_ptr
                        body.local_get(data_ptr_local);
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

                    // Call cel_create_string(data_ptr, len)
                    body.local_get(data_ptr_local); // Load data_ptr
                    body.i32_const(string_len); // Load length
                    body.call(env.create_string_func_id); // Returns *mut CelValue
                }
                CelVal::Bytes(bytes) => {
                    // Bytes literals require memory allocation (same pattern as strings)
                    let bytes_len = bytes.len() as i32;

                    // Create a local to store the bytes data pointer
                    let data_ptr_local = module.locals.add(ValType::I32);

                    // Allocate memory for the bytes data
                    body.i32_const(bytes_len)
                        .call(env.malloc_func_id) // Returns data_ptr
                        .local_set(data_ptr_local); // Store in local and pop from stack

                    // Get memory reference
                    let memory_id = module
                        .memories
                        .iter()
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("No memory found"))?
                        .id();

                    // Write each byte to the allocated memory
                    for (offset, &byte) in bytes.iter().enumerate() {
                        // Load data_ptr
                        body.local_get(data_ptr_local);
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

                    // Call cel_create_bytes(data_ptr, len)
                    body.local_get(data_ptr_local); // Load data_ptr
                    body.i32_const(bytes_len); // Load length
                    body.call(env.create_bytes_func_id); // Returns *mut CelValue
                }
                CelVal::Null => {
                    // Create a CelValue::Null pointer
                    body.call(env.create_null_func_id);
                }
                // Other literals not supported yet
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
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    body.call(env.add_func_id);
                }
                operators::SUBSTRACT => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Subtract operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    body.call(env.sub_func_id);
                }
                operators::MULTIPLY => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Multiply operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    body.call(env.mul_func_id);
                }
                operators::DIVIDE => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Divide operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    body.call(env.div_func_id);
                }
                operators::MODULO => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Modulo operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    body.call(env.mod_func_id);
                }
                operators::NEGATE => {
                    // Unary negation operator: -x
                    if call_expr.args.len() != 1 {
                        anyhow::bail!("Negation operator expects 1 argument");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    body.call(env.negate_func_id);
                }

                // Comparison operators
                operators::EQUALS => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Equals operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    body.call(env.eq_func_id);
                }
                operators::NOT_EQUALS => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Not equals operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    body.call(env.ne_func_id);
                }
                operators::GREATER => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Greater than operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    body.call(env.gt_func_id);
                }
                operators::LESS => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Less than operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    body.call(env.lt_func_id);
                }
                operators::GREATER_EQUALS => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Greater or equal operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    body.call(env.gte_func_id);
                }
                operators::LESS_EQUALS => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Less or equal operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    body.call(env.lte_func_id);
                }

                // Membership operator
                operators::IN => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("'in' operator expects 2 arguments");
                    }
                    // Left operand: element to search for
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    // Right operand: container (list or map)
                    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    // Call runtime function
                    body.call(env.in_func_id);
                }

                // Logical operators
                operators::LOGICAL_AND => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Logical AND operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    body.call(env.and_func_id);
                }
                operators::LOGICAL_OR => {
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Logical OR operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    body.call(env.or_func_id);
                }
                operators::LOGICAL_NOT => {
                    if call_expr.args.len() != 1 {
                        anyhow::bail!("Logical NOT operator expects 1 argument");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    body.call(env.not_func_id);
                }

                operators::NOT_STRICTLY_FALSE => {
                    // @not_strictly_false is used in comprehension loop conditions
                    // It returns true if the value is not exactly CelValue::Bool(false)
                    if call_expr.args.len() != 1 {
                        anyhow::bail!("@not_strictly_false expects 1 argument");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    body.call(env.not_strictly_false_func_id);
                }

                operators::CONDITIONAL => {
                    // Ternary/conditional operator: condition ? true_value : false_value
                    if call_expr.args.len() != 3 {
                        anyhow::bail!("Conditional operator expects 3 arguments");
                    }
                    // Compile all three arguments
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?; // condition
                    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?; // true_value
                    compile_expr(&call_expr.args[2].expr, body, env, ctx, module)?; // false_value
                    body.call(env.conditional_func_id);
                }

                // String functions
                "size" => {
                    // size() can work on strings, bytes, arrays, or maps
                    // For strings, it returns the number of Unicode codepoints
                    // For bytes, it returns the number of bytes
                    // For arrays, it returns the number of elements
                    // For maps, it returns the number of keys
                    if call_expr.args.len() != 1 {
                        anyhow::bail!("size() expects 1 argument");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;

                    // Call polymorphic cel_value_size which returns i64
                    // We need to convert it to *mut CelValue::Int
                    body.call(env.size_func_id); // Returns i64
                    body.call(env.create_int_func_id); // Convert i64 to *mut CelValue
                }

                "startsWith" => {
                    // Supports both: string.startsWith(prefix) and startsWith(string, prefix)
                    // Method syntax: target is Some, args has 1 element (the prefix)
                    // Function syntax: target is None, args has 2 elements (string, prefix)

                    if let Some(target) = &call_expr.target {
                        // Method syntax: "hello".startsWith("he")
                        if call_expr.args.len() != 1 {
                            anyhow::bail!("startsWith() method expects 1 argument");
                        }
                        // Compile the target string and the prefix argument
                        compile_expr(&target.expr, body, env, ctx, module)?;
                        compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    } else {
                        // Function syntax: startsWith("hello", "he")
                        if call_expr.args.len() != 2 {
                            anyhow::bail!("startsWith() function expects 2 arguments");
                        }
                        // Compile the string and the prefix
                        compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                        compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    }
                    // Call cel_string_starts_with which returns *mut CelValue::Bool
                    body.call(env.string_starts_with_func_id);
                }

                "endsWith" => {
                    // Supports both: string.endsWith(suffix) and endsWith(string, suffix)
                    // Method syntax: target is Some, args has 1 element (the suffix)
                    // Function syntax: target is None, args has 2 elements (string, suffix)

                    if let Some(target) = &call_expr.target {
                        // Method syntax: "hello".endsWith("lo")
                        if call_expr.args.len() != 1 {
                            anyhow::bail!("endsWith() method expects 1 argument");
                        }
                        // Compile the target string and the suffix argument
                        compile_expr(&target.expr, body, env, ctx, module)?;
                        compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    } else {
                        // Function syntax: endsWith("hello", "lo")
                        if call_expr.args.len() != 2 {
                            anyhow::bail!("endsWith() function expects 2 arguments");
                        }
                        // Compile the string and the suffix
                        compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                        compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    }
                    // Call cel_string_ends_with which returns *mut CelValue::Bool
                    body.call(env.string_ends_with_func_id);
                }

                "contains" => {
                    // Supports both: string.contains(substring) and contains(string, substring)
                    // Method syntax: target is Some, args has 1 element (the substring)
                    // Function syntax: target is None, args has 2 elements (string, substring)

                    if let Some(target) = &call_expr.target {
                        // Method syntax: "hello".contains("ll")
                        if call_expr.args.len() != 1 {
                            anyhow::bail!("contains() method expects 1 argument");
                        }
                        // Compile the target string and the substring argument
                        compile_expr(&target.expr, body, env, ctx, module)?;
                        compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    } else {
                        // Function syntax: contains("hello", "ll")
                        if call_expr.args.len() != 2 {
                            anyhow::bail!("contains() function expects 2 arguments");
                        }
                        // Compile the string and the substring
                        compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                        compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    }
                    // Call cel_string_contains which returns *mut CelValue::Bool
                    body.call(env.string_contains_func_id);
                }

                "matches" => {
                    // Supports both: string.matches(pattern) and matches(string, pattern)
                    // Method syntax: target is Some, args has 1 element (the pattern)
                    // Function syntax: target is None, args has 2 elements (string, pattern)

                    if let Some(target) = &call_expr.target {
                        // Method syntax: "foobar".matches("foo.*")
                        if call_expr.args.len() != 1 {
                            anyhow::bail!("matches() method expects 1 argument");
                        }
                        // Compile the target string and the pattern argument
                        compile_expr(&target.expr, body, env, ctx, module)?;
                        compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    } else {
                        // Function syntax: matches("foobar", "foo.*")
                        if call_expr.args.len() != 2 {
                            anyhow::bail!("matches() function expects 2 arguments");
                        }
                        // Compile the string and the pattern
                        compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                        compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    }
                    // Call cel_string_matches which returns *mut CelValue::Bool
                    body.call(env.string_matches_func_id);
                }

                // Temporal conversion functions
                "timestamp" => {
                    // timestamp(string) - parses RFC3339 timestamp string
                    // Returns *mut CelValue::Timestamp or Error
                    if call_expr.args.len() != 1 {
                        anyhow::bail!("timestamp() expects 1 argument (RFC3339 string)");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    body.call(env.timestamp_func_id);
                }

                "duration" => {
                    // duration(string) - parses CEL duration format string
                    // Returns *mut CelValue::Duration or Error
                    if call_expr.args.len() != 1 {
                        anyhow::bail!("duration() expects 1 argument (duration string)");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    body.call(env.duration_func_id);
                }

                "string" => {
                    // string(value) - converts any CelValue to string representation
                    // Returns *mut CelValue::String
                    if call_expr.args.len() != 1 {
                        anyhow::bail!("string() expects 1 argument");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    body.call(env.string_func_id);
                }

                "int" => {
                    // int(value) - converts value to int
                    // Returns *mut CelValue::Int
                    if call_expr.args.len() != 1 {
                        anyhow::bail!("int() expects 1 argument");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    body.call(env.int_func_id);
                }

                "uint" => {
                    // uint(value) - converts value to uint
                    // Returns *mut CelValue::UInt
                    if call_expr.args.len() != 1 {
                        anyhow::bail!("uint() expects 1 argument");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    body.call(env.uint_func_id);
                }

                "double" => {
                    // double(value) - converts value to double
                    // Returns *mut CelValue::Double
                    if call_expr.args.len() != 1 {
                        anyhow::bail!("double() expects 1 argument");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    body.call(env.double_func_id);
                }

                "bytes" => {
                    // bytes(value) - converts value to bytes
                    // Returns *mut CelValue::Bytes
                    if call_expr.args.len() != 1 {
                        anyhow::bail!("bytes() expects 1 argument");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    body.call(env.bytes_func_id);
                }

                "dyn" => {
                    // dyn(value) - identity function that marks value as dynamically typed
                    // In CEL, this is used to force dynamic dispatch for operations
                    // For our compiler, it's a no-op since we already do dynamic dispatch
                    if call_expr.args.len() != 1 {
                        anyhow::bail!("dyn() expects 1 argument");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    // No function call needed - just leave the value on the stack
                }

                operators::INDEX => {
                    // Index operator for arrays and maps
                    // array[index] or map[key]
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Index operator _[_] expects 2 arguments (container, index)");
                    }
                    // Compile container (array or map)
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    // Compile index (int for array, string for map)
                    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    // Call the polymorphic index function
                    body.call(env.index_func_id);
                }

                _ => anyhow::bail!("Unsupported function call: {}", call_expr.func_name),
            }
        }

        // 3. Identifiers (variables)
        Expr::Ident(name) => {
            // First check if this is a local variable (from comprehension scope)
            if let Some(&local_id) = ctx.local_vars.get(name) {
                // This is a local variable, load it from the local
                body.local_get(local_id);
            } else {
                // Not a local variable, check global variables
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
        }

        // 4. Field selection (e.g., object.field, input.user.name)
        Expr::Select(select_expr) => {
            // Recursively compile the operand as a pointer (e.g., `input` or `input.user`)
            // This will leave a *mut CelValue (i32) on the stack
            compile_expr(&select_expr.operand.expr, body, env, ctx, module)?;

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

            // Now call the appropriate field function
            // Stack currently has: [obj_ptr, field_name_ptr]
            // We need: [obj_ptr, field_name_ptr, field_len]
            body.i32_const(field_len);

            // Call appropriate function based on whether this is has() macro
            if select_expr.test {
                // has() macro: check field existence, returns Bool
                body.call(env.has_field_func_id);
            } else {
                // Normal field access: get field value
                body.call(env.get_field_func_id);
            }
        }

        // 5. List Literals
        // [1, 2, 3] creates a CelValue::Array
        Expr::List(list_expr) => {
            // Create an empty array
            body.call(env.create_array_func_id);

            // Create a local to hold the array pointer while we push elements
            let array_ptr_local = module.locals.add(ValType::I32);
            body.local_set(array_ptr_local); // Pop array pointer into local

            // For each element in the list
            for element in &list_expr.elements {
                // Compile the element expression (leaves *mut CelValue on stack)
                compile_expr(&element.expr, body, env, ctx, module)?;

                // Create a local to hold the element pointer
                let element_ptr_local = module.locals.add(ValType::I32);
                body.local_set(element_ptr_local); // Pop element pointer into local

                // Push: cel_array_push(array_ptr, element_ptr)
                body.local_get(array_ptr_local); // Load array pointer
                body.local_get(element_ptr_local); // Load element pointer
                body.call(env.array_push_func_id); // Call push (returns void)
            }

            // Leave the array pointer on the stack
            body.local_get(array_ptr_local);
        }

        // 5b. Map Literals
        // {"key": "value", "other": 123} creates a CelValue::Object (HashMap)
        Expr::Map(map_expr) => {
            use cel::common::ast::EntryExpr;

            // Create an empty map
            body.call(env.create_map_func_id);

            // Create a local to hold the map pointer while we insert entries
            let map_ptr_local = module.locals.add(ValType::I32);
            body.local_set(map_ptr_local); // Pop map pointer into local

            // For each entry in the map
            for entry in &map_expr.entries {
                match &entry.expr {
                    EntryExpr::MapEntry(map_entry) => {
                        // Compile the key expression (leaves *mut CelValue on stack)
                        compile_expr(&map_entry.key.expr, body, env, ctx, module)?;

                        // Create a local to hold the key pointer
                        let key_ptr_local = module.locals.add(ValType::I32);
                        body.local_set(key_ptr_local); // Pop key pointer into local

                        // Compile the value expression (leaves *mut CelValue on stack)
                        compile_expr(&map_entry.value.expr, body, env, ctx, module)?;

                        // Create a local to hold the value pointer
                        let value_ptr_local = module.locals.add(ValType::I32);
                        body.local_set(value_ptr_local); // Pop value pointer into local

                        // Insert: cel_map_insert(map_ptr, key_ptr, value_ptr)
                        body.local_get(map_ptr_local); // Load map pointer
                        body.local_get(key_ptr_local); // Load key pointer
                        body.local_get(value_ptr_local); // Load value pointer
                        body.call(env.map_insert_func_id); // Call insert (returns void)
                    }
                    _ => anyhow::bail!("Unsupported map entry type: {:?}", entry.expr),
                }
            }

            // Leave the map pointer on the stack
            body.local_get(map_ptr_local);
        }

        // 6. Comprehensions (e.g., [1,2,3].all(x, x > 0))
        // The CEL parser automatically expands macros like all() into Comprehension expressions
        Expr::Comprehension(comp_expr) => {
            // For all() macro:
            // - accu_var is "@result" (accumulator, starts as true)
            // - iter_var is the iteration variable (e.g., "x")
            // - iter_range is the array expression
            // - loop_cond checks if we should continue (e.g., @not_strictly_false(@result))
            // - loop_step updates the accumulator (e.g., @result && predicate)
            // - result is what we return (e.g., @result)

            // Step 1: Compile the iter_range (the array to iterate over)
            compile_expr(&comp_expr.iter_range.expr, body, env, ctx, module)?;

            // Create a local to hold the array pointer
            let array_ptr_local = module.locals.add(ValType::I32);
            body.local_set(array_ptr_local);

            // Step 2: Get the array length
            body.local_get(array_ptr_local);
            body.call(env.array_len_func_id); // Returns i32 length

            // Create a local to hold the length
            let length_local = module.locals.add(ValType::I32);
            body.local_set(length_local);

            // Step 3: Initialize the accumulator variable
            // Compile accu_init (e.g., CelValue::Bool(true) for all())
            compile_expr(&comp_expr.accu_init.expr, body, env, ctx, module)?;

            // Create a local to hold the accumulator pointer
            let accu_local = module.locals.add(ValType::I32);
            body.local_set(accu_local);

            // Step 4: Initialize loop counter (index = 0)
            let index_local = module.locals.add(ValType::I32);
            body.i32_const(0);
            body.local_set(index_local);

            // Step 5: Create the loop using WASM block/loop instructions
            // Structure: block $exit { loop $continue { ... } }
            let exit_block = body.dangling_instr_seq(None);
            let exit_block_id = exit_block.id();
            let continue_loop = body.dangling_instr_seq(None);
            let continue_loop_id = continue_loop.id();

            body.instr(walrus::ir::Block { seq: exit_block_id });

            // Start of exit block
            body.instr_seq(exit_block_id).instr(walrus::ir::Loop {
                seq: continue_loop_id,
            });

            // Start of continue loop
            let mut loop_body = body.instr_seq(continue_loop_id);

            // Check if index >= length (exit condition)
            loop_body.local_get(index_local);
            loop_body.local_get(length_local);
            loop_body.binop(walrus::ir::BinaryOp::I32GeU); // index >= length?
            loop_body.instr(walrus::ir::BrIf {
                block: exit_block_id,
            }); // Exit if true

            // Get the current element: cel_array_get(array_ptr, index)
            loop_body.local_get(array_ptr_local);
            loop_body.local_get(index_local);
            loop_body.call(env.array_get_func_id); // Returns *mut CelValue

            // Create a local for the current element
            let element_local = module.locals.add(ValType::I32);
            loop_body.local_set(element_local);

            // Create a new context with the iteration variable bound to the element
            let inner_ctx = ctx.with_local(comp_expr.iter_var.clone(), element_local);

            // Also bind the accumulator variable to the context
            let inner_ctx = inner_ctx.with_local(comp_expr.accu_var.clone(), accu_local);

            // Compile the loop_step (e.g., @result && predicate)
            // This updates the accumulator
            compile_expr(
                &comp_expr.loop_step.expr,
                &mut loop_body,
                env,
                &inner_ctx,
                module,
            )?;

            // Store the new accumulator value
            loop_body.local_set(accu_local);

            // Check the loop_cond to see if we should short-circuit
            // For all(), this is: @not_strictly_false(@result)
            compile_expr(
                &comp_expr.loop_cond.expr,
                &mut loop_body,
                env,
                &inner_ctx,
                module,
            )?;

            // Convert the loop condition (CelValue::Bool) to i64 (0 or 1)
            loop_body.call(env.value_to_bool_func_id); // Returns i64: 1 if true, 0 if false

            // If the condition is false (0), we should exit the loop
            // i32.eqz checks if value is 0, returns 1 if yes, 0 if no
            loop_body.unop(walrus::ir::UnaryOp::I64Eqz); // 1 if cond was false, 0 if true
            loop_body.instr(walrus::ir::BrIf {
                block: exit_block_id,
            }); // Exit if condition was false

            // Increment the index
            loop_body.local_get(index_local);
            loop_body.i32_const(1);
            loop_body.binop(walrus::ir::BinaryOp::I32Add);
            loop_body.local_set(index_local);

            // Continue the loop
            loop_body.instr(walrus::ir::Br {
                block: continue_loop_id,
            }); // Jump back to start of loop

            // After the loop, compile the result expression
            // For all(), this is just @result (the accumulator)
            let result_ctx = ctx.with_local(comp_expr.accu_var.clone(), accu_local);
            compile_expr(&comp_expr.result.expr, body, env, &result_ctx, module)?;
        }

        _ => anyhow::bail!("Unsupported expression type: {:?}", expr),
    }

    Ok(())
}
