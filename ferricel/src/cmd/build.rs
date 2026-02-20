use std::path::Path;

use cel::parser::Parser;
use walrus::{FunctionBuilder, ModuleConfig, ValType};

use crate::compiler;

// Embed the runtime WASM at compile time
const RUNTIME_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../target/wasm32-unknown-unknown/release/runtime.wasm"
));

pub fn run(cel_code: &str, output_path: &Path) -> Result<(), anyhow::Error> {
    // 1. Load the runtime template from embedded bytes
    let mut module = ModuleConfig::new().parse(RUNTIME_BYTES)?;

    let env = compiler::CompilerEnv {
        add_func_id: module.exports.get_func("cel_int_add")?,
    };

    // (Optional) Remove the helper from exports so the Host can't call it directly
    module.exports.remove("cel_int_add")?;

    // 2. Parse the CEL expression
    let root_ast = Parser::default()
        .parse(cel_code)
        .map_err(|e| anyhow::anyhow!("Parse error: {:?}", e))?;

    // 3. Build the 'validate' function (i32, i32) -> i32
    let mut validate_func = FunctionBuilder::new(
        &mut module.types,
        &[ValType::I32, ValType::I32],
        &[ValType::I32],
    );
    let input_ptr_arg = module.locals.add(ValType::I32);
    let data_ptr_arg = module.locals.add(ValType::I32);

    let mut body = validate_func.func_body();

    // 4. Walk the AST. We pass `.expr` to extract the Expr enum from the root IdedExpr
    compiler::compile_expr(&root_ast.expr, &mut body, &env)?;

    // 5. Finish the function definition
    let validate_id = validate_func.finish(vec![input_ptr_arg, data_ptr_arg], &mut module.funcs);

    // 6. Export the 'validate' function for the Host
    module.exports.add("validate", validate_id);

    // 7. Write the merged WebAssembly file
    module.emit_wasm_file(output_path)?;

    println!("Successfully compiled CEL into: {}", output_path.display());
    Ok(())
}
