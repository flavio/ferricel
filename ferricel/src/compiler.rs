use cel::common::ast::operators;
use cel::common::ast::Expr;
use cel::common::value::CelVal;
use walrus::{FunctionId, InstrSeqBuilder};

// A struct to hold the handles to your runtime functions
pub struct CompilerEnv {
    pub add_func_id: FunctionId,
    // You will eventually add more here:
    // pub get_field_id: FunctionId,
    // pub string_eq_id: FunctionId,
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
