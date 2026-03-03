use anyhow::Result;
use cel::parser::Parser;

fn main() -> Result<()> {
    let programs = vec![
        "[1, 2, 3]",
        "data.items",
        "[1, 2, 3][0]", // index access
        "[1, 2, 3].size()",
    ];

    for program in programs {
        println!("\n============================================================");
        println!("Parsing: {}", program);
        println!("============================================================");
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
