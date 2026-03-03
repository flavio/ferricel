use anyhow::Result;
use cel::common::ast::Expr;
use cel::parser::Parser;

fn main() -> Result<()> {
    // Simple all() test
    let code = "[1, 2, 3].all(x, x > 0)";

    println!("Parsing: {}", code);
    let parsed = Parser::default().parse(code)?;

    if let Expr::Comprehension(comp) = &parsed.expr {
        println!("\n=== Comprehension Structure ===");
        println!("iter_range: {:#?}", comp.iter_range);
        println!("\niter_var: {:?}", comp.iter_var);
        println!("iter_var2: {:?}", comp.iter_var2);
        println!("accu_var: {:?}", comp.accu_var);
        println!("\naccu_init: {:#?}", comp.accu_init);
        println!("\nloop_cond: {:#?}", comp.loop_cond);
        println!("\nloop_step: {:#?}", comp.loop_step);
        println!("\nresult: {:#?}", comp.result);
    } else {
        println!("Not a comprehension!");
    }

    Ok(())
}
