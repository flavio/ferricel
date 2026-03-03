use anyhow::Result;
use cel::common::ast::Expr;
use cel::parser::Parser;

fn main() -> Result<()> {
    for program in ["has(input.field)", "input.field"] {
        println!("\n=== {} ===", program);
        match Parser::default().parse(program) {
            Ok(expr) => {
                if let Expr::Select(select) = &expr.expr {
                    println!("Field: {}", select.field);
                    println!("Test flag: {}", select.test);
                }
            }
            Err(e) => println!("Error: {}", e),
        }
    }
    Ok(())
}
