use anyhow::Result;

fn main() -> Result<()> {
    // Try to compile a CEL expression and get the AST
    let code = "10 + 20";

    // Using Parser
    let parsed = cel::parser::Parser::default().parse(code)?;
    println!("Parsed with Parser: {:?}", parsed);
    println!("Has .expr: {:?}", parsed.expr);

    Ok(())
}
