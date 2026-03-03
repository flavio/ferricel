use anyhow::Result;
use cel::parser::Parser;

fn main() -> Result<()> {
    let programs = vec![
        // all() - should return Comprehension
        "[1, 2, 3].all(x, x > 0)",
        // exists() - should return Comprehension
        "[1, 2, 3].exists(x, x > 2)",
        // exists_one() - should return Comprehension
        "[1, 2, 3].exists_one(x, x == 2)",
        // map() - should return Comprehension
        "[1, 2, 3].map(x, x * 2)",
        // filter() - should return Comprehension
        "[1, 2, 3].filter(x, x > 1)",
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
