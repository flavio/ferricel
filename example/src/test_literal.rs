use anyhow::Result;
use cel::common::ast::Expr;
use cel::parser::Parser;

fn main() -> Result<()> {
    let expr = Parser::default().parse("42")?;

    match &expr.expr {
        Expr::Literal(lit) => {
            println!("Literal type: {:?}", std::any::type_name_of_val(lit));
            println!("Literal value: {:?}", lit);
        }
        _ => println!("Not a literal"),
    }

    Ok(())
}
