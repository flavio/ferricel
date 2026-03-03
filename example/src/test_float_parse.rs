use anyhow::Result;
use cel::common::ast::Expr;
use cel::parser::Parser;

fn main() -> Result<()> {
    let tests = vec!["42", "3.14", "1.5", "100", "0.5", "-3.14", "1.0"];

    for test in tests {
        match Parser::default().parse(test) {
            Ok(expr) => match &expr.expr {
                Expr::Literal(lit) => {
                    println!("{:8} => {:?}", test, lit);
                }
                _ => println!("{:8} => Not a literal", test),
            },
            Err(e) => println!("{:8} => Parse error: {:?}", test, e),
        }
    }

    Ok(())
}
