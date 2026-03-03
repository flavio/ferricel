use anyhow::Result;
use cel::parser::Parser;

fn main() -> Result<()> {
    for program in ["has(input.field)", "has(input.user.name)", "input.field"] {
        println!("\n=== Parsing: {} ===", program);
        match Parser::default().parse(program) {
            Ok(expr) => println!("{:#?}", expr),
            Err(e) => println!("Error: {}", e),
        }
    }
    Ok(())
}
