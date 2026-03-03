use anyhow::Result;
use cel::parser::Parser;

fn dump_expression(program: &str) -> Result<()> {
    let expr = Parser::default().parse(program)?;
    println!("Parsed expression: {:?}", expr);
    Ok(())
}

fn main() -> Result<()> {
    for program in [
        "42",
        "true",
        "(10  == 20)",
        "10 > 5",
        "object.metadata.name.contains('demo')",
    ] {
        println!("Parsing program: {}", program);
        dump_expression(program)?;
    }
    Ok(())
}
