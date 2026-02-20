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
    pub add_func_id: FunctionId,
    // You will eventually add more here:
    // pub get_field_id: FunctionId,
    // pub string_eq_id: FunctionId,
}

/// Compile a CEL expression into a WebAssembly module
///
/// Takes a CEL expression string and returns the compiled WASM module as bytes.
/// The resulting module exports a `validate` function with signature (i32, i32) -> i64.
pub fn compile_cel_to_wasm(cel_code: &str) -> Result<Vec<u8>, anyhow::Error> {
    // 1. Load the runtime template from embedded bytes
    let mut module = ModuleConfig::new().parse(RUNTIME_BYTES)?;

    // 2. Set up the compiler environment
    let env = CompilerEnv {
        add_func_id: module.exports.get_func("cel_int_add")?,
    };

    // 3. Remove the helper from exports so the Host can't call it directly
    module.exports.remove("cel_int_add")?;

    // 4. Parse the CEL expression
    let root_ast = Parser::default()
        .parse(cel_code)
        .map_err(|e| anyhow::anyhow!("Parse error: {:?}", e))?;

    // 5. Build the 'validate' function (i32, i32) -> i64
    let mut validate_func = FunctionBuilder::new(
        &mut module.types,
        &[ValType::I32, ValType::I32],
        &[ValType::I64],
    );
    let input_ptr_arg = module.locals.add(ValType::I32);
    let data_ptr_arg = module.locals.add(ValType::I32);

    let mut body = validate_func.func_body();

    // 6. Walk the AST and compile to WASM instructions
    compile_expr(&root_ast.expr, &mut body, &env)?;

    // 7. Finish the function definition
    let validate_id = validate_func.finish(vec![input_ptr_arg, data_ptr_arg], &mut module.funcs);

    // 8. Export the 'validate' function for the Host
    module.exports.add("validate", validate_id);

    // 9. Emit the module as bytes
    let wasm_bytes = module.emit_wasm();

    Ok(wasm_bytes)
}

// We pass the inner `Expr` enum to this recursive function
pub fn compile_expr(
    expr: &Expr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
) -> Result<(), anyhow::Error> {
    match expr {
        // 1. Literal Values
        Expr::Literal(literal) => {
            match literal {
                CelVal::Int(value) => {
                    // Push the integer onto the WASM stack
                    body.i64_const(*value);
                }
                CelVal::Boolean(b) => {
                    // WASM doesn't have a boolean type, we use i32 (1 or 0)
                    body.i32_const(if *b { 1 } else { 0 });
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
                operators::ADD => {
                    // Addition: compile both arguments
                    if call_expr.args.len() != 2 {
                        anyhow::bail!("Add operator expects 2 arguments");
                    }
                    compile_expr(&call_expr.args[0].expr, body, env)?;
                    compile_expr(&call_expr.args[1].expr, body, env)?;
                    body.call(env.add_func_id);
                }
                operators::SUBSTRACT => {
                    anyhow::bail!("Subtraction not yet implemented in runtime!");
                }
                operators::EQUALS
                | operators::GREATER
                | operators::LESS
                | operators::GREATER_EQUALS
                | operators::LESS_EQUALS
                | operators::NOT_EQUALS => {
                    anyhow::bail!("Relational operators not yet implemented");
                }
                _ => anyhow::bail!("Unsupported function call: {}", call_expr.func_name),
            }
        }

        // 3. Identifiers (variables)
        Expr::Ident(name) => {
            anyhow::bail!("Variable access not yet implemented: {}", name);
        }

        // 4. Field selection (e.g., object.field)
        Expr::Select(_select_expr) => {
            anyhow::bail!("Field selection not yet implemented");
        }

        _ => anyhow::bail!("Unsupported expression type: {:?}", expr),
    }

    Ok(())
}
