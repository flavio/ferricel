use anyhow::Result;
use cel::common::ast::Expr;
use cel::parser::Parser;

fn main() -> Result<()> {
    let expr = Parser::default().parse("42")?;
    println!("Expression type: {:?}", std::any::type_name_of_val(&expr));
    println!(
        "Inner expr type: {:?}",
        std::any::type_name_of_val(&expr.expr)
    );

    // Try to match on the expr
    match &expr.expr {
        Expr::Literal(lit) => {
            println!("Literal: {:?}", lit);
        }
        _ => println!("Other"),
    }

    Ok(())
}
