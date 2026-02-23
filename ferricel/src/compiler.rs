use cel::common::ast::operators;
use cel::common::ast::Expr;
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

    // Memory allocation (for field names)
    pub malloc_func_id: FunctionId,

    // Value creation helpers
    pub create_int_func_id: FunctionId,
    pub create_bool_func_id: FunctionId,

    // Value conversion helpers
    pub value_to_bool_func_id: FunctionId,
}

/// Compilation context that holds state during expression compilation
/// This includes local variable bindings for comprehensions and other scoped contexts
pub struct CompilerContext {
    /// Maps variable names to their local IDs in the WASM function
    /// Used for iteration variables in comprehensions (e.g., "x" in [1,2,3].all(x, x > 0))
    pub local_vars: HashMap<String, LocalId>,
}

impl CompilerContext {
    /// Create a new empty compilation context
    pub fn new() -> Self {
        Self {
            local_vars: HashMap::new(),
        }
    }

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

        // Memory allocation
        malloc_func_id,

        // Value creation helpers
        create_int_func_id: module.exports.get_func("cel_create_int")?,
        create_bool_func_id: module.exports.get_func("cel_create_bool")?,

        // Value conversion helpers
        value_to_bool_func_id: module.exports.get_func("cel_value_to_bool")?,
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
    module.exports.remove("cel_create_int")?;
    module.exports.remove("cel_create_bool")?;
    module.exports.remove("cel_value_to_bool")?;

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
    let ctx = CompilerContext::new();
    compile_expr(&root_ast.expr, &mut body, &env, &ctx, &mut module)?;

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
    // List Literal Tests
    // ========================================

    #[rstest]
    #[case::empty("[]", "[]")]
    #[case::single_element("[42]", "[42]")]
    #[case::multiple_integers("[1, 2, 3]", "[1,2,3]")]
    #[case::with_expressions("[1 + 1, 2 * 3, 10 - 5]", "[2,6,5]")]
    #[case::mixed_types("[1, true, 3, false]", "[1,true,3,false]")]
    #[case::with_comparisons("[5 > 3, 2 < 1, 10 == 10]", "[true,false,true]")]
    #[case::concatenation("[1, 2] + [3, 4]", "[1,2,3,4]")]
    #[case::concatenation_empty("[] + []", "[]")]
    #[case::concatenation_with_empty("[1, 2, 3] + []", "[1,2,3]")]
    fn test_list_literals(#[case] expr: &str, #[case] expected: &str) {
        let wasm_bytes = compile_cel_to_wasm(expr).expect("Failed to compile");
        let json_result =
            runtime::execute_wasm_with_vars(&wasm_bytes, None, None).expect("Failed to execute");
        assert_eq!(json_result, expected);
    }

    // ========================================
    // all() Macro Tests
    // ========================================

    #[rstest]
    #[case::all_true("[1, 2, 3].all(x, x > 0)", "true")]
    #[case::some_false("[1, -2, 3].all(x, x > 0)", "false")]
    #[case::empty_list("[].all(x, x > 0)", "true")]
    #[case::complex_predicate("[10, 20, 30].all(x, x >= 10 && x <= 30)", "true")]
    #[case::equality("[5, 5, 5].all(x, x == 5)", "true")]
    #[case::single_false("[1, 2, 3, 0].all(x, x > 0)", "false")]
    #[case::with_expressions("[1+1, 2*3, 10-5].all(x, x > 1)", "true")]
    fn test_all_macro(#[case] expr: &str, #[case] expected: &str) {
        let wasm_bytes = compile_cel_to_wasm(expr).expect("Failed to compile");
        let json_result =
            runtime::execute_wasm_with_vars(&wasm_bytes, None, None).expect("Failed to execute");
        assert_eq!(json_result, expected);
    }

    // ========================================
    // exists() Macro Tests
    // ========================================

    #[rstest]
    #[case::one_true("[1, 2, 3].exists(x, x > 2)", "true")]
    #[case::all_false("[1, 2, 3].exists(x, x > 10)", "false")]
    #[case::empty_list("[].exists(x, x > 0)", "false")]
    #[case::all_true("[5, 10, 15].exists(x, x > 0)", "true")]
    #[case::complex_predicate("[1, 5, 10].exists(x, x >= 5 && x <= 10)", "true")]
    #[case::first_element_true("[10, 1, 2].exists(x, x > 5)", "true")]
    #[case::last_element_true("[1, 2, 10].exists(x, x > 5)", "true")]
    fn test_exists_macro(#[case] expr: &str, #[case] expected: &str) {
        let wasm_bytes = compile_cel_to_wasm(expr).expect("Failed to compile");
        let json_result =
            runtime::execute_wasm_with_vars(&wasm_bytes, None, None).expect("Failed to execute");
        assert_eq!(json_result, expected);
    }

    // ========================================
    // exists_one() Macro Tests
    // ========================================

    #[rstest]
    #[case::exactly_one("[1, 5, 3].exists_one(x, x > 4)", "true")]
    #[case::none("[1, 2, 3].exists_one(x, x > 10)", "false")]
    #[case::multiple("[5, 10, 15].exists_one(x, x > 4)", "false")]
    #[case::empty_list("[].exists_one(x, x > 0)", "false")]
    #[case::first_element_only("[10, 1, 2].exists_one(x, x > 5)", "true")]
    #[case::last_element_only("[1, 2, 10].exists_one(x, x > 5)", "true")]
    #[case::two_elements("[10, 20, 1].exists_one(x, x > 5)", "false")]
    fn test_exists_one_macro(#[case] expr: &str, #[case] expected: &str) {
        let wasm_bytes = compile_cel_to_wasm(expr).expect("Failed to compile");
        let json_result =
            runtime::execute_wasm_with_vars(&wasm_bytes, None, None).expect("Failed to execute");
        assert_eq!(json_result, expected);
    }

    // ========================================
    // filter() Macro Tests
    // ========================================

    #[rstest]
    #[case::basic("[1, 2, 3, 4, 5].filter(x, x > 2)", "[3,4,5]")]
    #[case::none_match("[1, 2, 3].filter(x, x > 10)", "[]")]
    #[case::all_match("[1, 2, 3].filter(x, x > 0)", "[1,2,3]")]
    #[case::empty_list("[].filter(x, x > 0)", "[]")]
    #[case::even_numbers("[1, 2, 3, 4, 5, 6].filter(x, x % 2 == 0)", "[2,4,6]")]
    #[case::complex_predicate("[1, 5, 10, 15, 20].filter(x, x >= 5 && x <= 15)", "[5,10,15]")]
    #[case::first_element_only("[10, 1, 2, 3].filter(x, x > 5)", "[10]")]
    #[case::last_element_only("[1, 2, 3, 10].filter(x, x > 5)", "[10]")]
    fn test_filter_macro(#[case] expr: &str, #[case] expected: &str) {
        let wasm_bytes = compile_cel_to_wasm(expr).expect("Failed to compile");
        let json_result =
            runtime::execute_wasm_with_vars(&wasm_bytes, None, None).expect("Failed to execute");
        assert_eq!(json_result, expected);
    }

    // ========================================
    // map() Macro Tests
    // ========================================

    #[rstest]
    #[case::basic("[1, 2, 3].map(x, x * 2)", "[2,4,6]")]
    #[case::empty_list("[].map(x, x * 2)", "[]")]
    #[case::identity("[1, 2, 3].map(x, x)", "[1,2,3]")]
    #[case::addition("[1, 2, 3].map(x, x + 10)", "[11,12,13]")]
    #[case::square("[1, 2, 3, 4].map(x, x * x)", "[1,4,9,16]")]
    #[case::type_change("[1, 2, 3].map(x, x > 1)", "[false,true,true]")]
    #[case::division("[10, 20, 30].map(x, x / 10)", "[1,2,3]")]
    #[case::complex_expression("[1, 2, 3].map(x, (x * 2) + 1)", "[3,5,7]")]
    #[case::single_element("[5].map(x, x * 2)", "[10]")]
    #[case::negative_numbers("[-1, -2, -3].map(x, x * -1)", "[1,2,3]")]
    #[case::modulo("[10, 11, 12].map(x, x % 3)", "[1,2,0]")]
    fn test_map_macro(#[case] expr: &str, #[case] expected: &str) {
        let wasm_bytes = compile_cel_to_wasm(expr).expect("Failed to compile");
        let json_result =
            runtime::execute_wasm_with_vars(&wasm_bytes, None, None).expect("Failed to execute");
        assert_eq!(json_result, expected);
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

    // ============================================================================
    // HAS MACRO TESTS
    // ============================================================================

    #[rstest]
    #[case(r#"{"name": "Alice", "age": 30}"#, "has(input.name)", 1)]
    #[case(r#"{"name": "Alice", "age": 30}"#, "has(input.age)", 1)]
    #[case(r#"{"name": "Alice"}"#, "has(input.age)", 0)]
    #[case(r#"{"name": "Alice"}"#, "has(input.email)", 0)]
    #[case(r#"{}"#, "has(input.anything)", 0)]
    fn test_has_macro_basic(#[case] input_json: &str, #[case] expr: &str, #[case] expected: i64) {
        let result =
            compile_and_execute_with_vars(expr, Some(input_json), None).expect("Failed to execute");
        assert_eq!(
            result, expected,
            "Expression '{}' with input {} should evaluate to {}",
            expr, input_json, expected
        );
    }

    #[rstest]
    #[case(r#"{"user": {"name": "Bob"}}"#, "has(input.user.name)", 1)]
    #[case(r#"{"user": {"name": "Bob"}}"#, "has(input.user.age)", 0)]
    #[case(r#"{"user": {}}"#, "has(input.user.name)", 0)]
    #[case(r#"{"a": {"b": {"c": 42}}}"#, "has(input.a.b.c)", 1)]
    #[case(r#"{"a": {"b": {}}}"#, "has(input.a.b.c)", 0)]
    fn test_has_macro_nested(#[case] input_json: &str, #[case] expr: &str, #[case] expected: i64) {
        let result =
            compile_and_execute_with_vars(expr, Some(input_json), None).expect("Failed to execute");
        assert_eq!(
            result, expected,
            "Expression '{}' with input {} should evaluate to {}",
            expr, input_json, expected
        );
    }

    #[test]
    fn test_has_macro_with_data_variable() {
        let data_json = r#"{"config": {"enabled": true}}"#;
        let result = compile_and_execute_with_vars("has(data.config)", None, Some(data_json))
            .expect("Failed to execute");
        assert_eq!(result, 1, "has(data.config) should return true");
    }

    #[test]
    fn test_has_macro_with_null_value() {
        // Field exists but value is null - should return true
        let input_json = r#"{"nullable": null}"#;
        let result = compile_and_execute_with_vars("has(input.nullable)", Some(input_json), None)
            .expect("Failed to execute");
        assert_eq!(
            result, 1,
            "has(input.nullable) should return true even when value is null"
        );
    }

    #[rstest]
    #[case(r#"{"age": 25}"#, "has(input.age) && input.age > 18", 1)]
    #[case(r#"{"age": 15}"#, "has(input.age) && input.age > 18", 0)]
    #[case(r#"{"age": 25}"#, "has(input.age) || has(input.name)", 1)]
    #[case(r#"{}"#, "has(input.age) || has(input.name)", 0)]
    #[case(r#"{"name": "Alice"}"#, "!has(input.age)", 1)]
    #[case(r#"{"age": 25}"#, "!has(input.missing)", 1)]
    fn test_has_macro_in_expressions(
        #[case] input_json: &str,
        #[case] expr: &str,
        #[case] expected: i64,
    ) {
        let result =
            compile_and_execute_with_vars(expr, Some(input_json), None).expect("Failed to execute");
        assert_eq!(
            result, expected,
            "Expression '{}' with input {} should evaluate to {}",
            expr, input_json, expected
        );
    }

    #[rstest]
    #[case(r#"{"a": 1, "b": 2}"#, "has(input.a) && has(input.b)", 1)]
    #[case(
        r#"{"a": 1, "b": 2}"#,
        "has(input.a) && has(input.b) && !has(input.c)",
        1
    )]
    #[case(r#"{"a": 1}"#, "has(input.a) && has(input.b)", 0)]
    #[case(
        r#"{"a": 1, "b": 2, "c": 3}"#,
        "has(input.a) && has(input.b) && has(input.c)",
        1
    )]
    fn test_has_macro_multiple_fields(
        #[case] input_json: &str,
        #[case] expr: &str,
        #[case] expected: i64,
    ) {
        let result =
            compile_and_execute_with_vars(expr, Some(input_json), None).expect("Failed to execute");
        assert_eq!(
            result, expected,
            "Expression '{}' with input {} should evaluate to {}",
            expr, input_json, expected
        );
    }
}
