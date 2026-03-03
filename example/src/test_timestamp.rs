use anyhow::Result;
use cel::parser::Parser;
use cel::common::ast::Expr;

fn main() -> Result<()> {
    // Test parsing a timestamp function call
    let expr = Parser::default().parse("timestamp('2023-05-28T00:00:00Z')")?;
    
    println!("Parsed expression: {:#?}", expr);
    println!("\nExpression type: {:?}", std::any::type_name_of_val(&expr.expr));
    
    // Check the inner expression
    match &expr.expr {
        Expr::Call(call_expr) => {
            println!("\nFunction call details:");
            println!("  Function name: {}", call_expr.func_name);
            println!("  Number of arguments: {}", call_expr.args.len());
            if !call_expr.args.is_empty() {
                println!("  First argument: {:#?}", call_expr.args[0]);
            }
        }
        _ => println!("Not a function call"),
    }
    
    Ok(())
}
