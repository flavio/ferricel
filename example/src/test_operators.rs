use anyhow::Result;
use cel::common::ast::operators::*;

fn main() -> Result<()> {
    println!("Testing operator constants:");
    println!("ADD: {}", ADD);
    println!("SUBSTRACT: {}", SUBSTRACT);
    println!("EQUALS: {}", EQUALS);
    println!("GREATER: {}", GREATER);
    println!("LESS: {}", LESS);
    println!("GREATER_EQUALS: {}", GREATER_EQUALS);
    println!("LESS_EQUALS: {}", LESS_EQUALS);
    println!("NOT_EQUALS: {}", NOT_EQUALS);

    Ok(())
}
