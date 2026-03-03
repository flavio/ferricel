use anyhow::Result;
use cel::parser::Parser;

fn main() -> Result<()> {
    // Test all() macro
    let programs = vec![
        "[1, 2, 3].all(x, x > 0)",
        "data.items.all(item, item.count > 0)",
        "[true, false, true].all(b, b)",
    ];

    for program in programs {
        println!("\n=== Parsing: {} ===", program);
        match Parser::default().parse(program) {
            Ok(expr) => {
                println!("{:#?}", expr);
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
    }

    Ok(())
}
