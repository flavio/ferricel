use cel::common::ast::LiteralValue;
use ferricel_types::functions::RuntimeFunction;
use walrus::{InstrSeqBuilder, LocalId, ValType};

use super::{context::CompilerEnv, helpers::get_memory_id};

/// Compile a literal CEL value into WASM instructions.
/// Leaves a *mut CelValue (i32) on the stack.
pub fn compile_literal(
    literal: &LiteralValue,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match literal {
        LiteralValue::Int(value) => {
            // Create a CelValue::Int pointer
            body.i64_const(**value);
            body.call(env.get(RuntimeFunction::CreateInt));
        }
        LiteralValue::UInt(value) => {
            // Create a CelValue::UInt pointer
            // Note: WASM only has i64, so we pass u64 as i64
            body.i64_const(**value as i64);
            body.call(env.get(RuntimeFunction::CreateUint));
        }
        LiteralValue::Boolean(b) => {
            // Create a CelValue::Bool pointer
            body.i64_const(if **b { 1 } else { 0 });
            body.call(env.get(RuntimeFunction::CreateBool));
        }
        LiteralValue::Double(d) => {
            // Create a CelValue::Double pointer
            body.f64_const(**d);
            body.call(env.get(RuntimeFunction::CreateDouble));
        }
        LiteralValue::String(s) => {
            // String literals can reuse compile_string_to_local but we need to leave
            // the result on the stack (not in a local). We use compile_string_to_local
            // and then push the local value back.
            let cel_val_local = compile_string_literal(s.inner(), body, env, module)?;
            body.local_get(cel_val_local);
        }
        LiteralValue::Bytes(bytes) => {
            // Bytes literals require memory allocation (same pattern as strings)
            let cel_val_local = compile_bytes_literal(bytes.inner(), body, env, module)?;
            body.local_get(cel_val_local);
        }
        LiteralValue::Null => {
            // Create a CelValue::Null pointer
            body.call(env.get(RuntimeFunction::CreateNull));
        }
    }
    Ok(())
}

/// Compile a string literal, returning the LocalId of the resulting CelValue pointer.
fn compile_string_literal(
    s: &str,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    module: &mut walrus::Module,
) -> Result<LocalId, anyhow::Error> {
    let string_bytes = s.as_bytes();
    let string_len = string_bytes.len() as i32;

    // Create a local to store the string data pointer
    let data_ptr_local = module.locals.add(ValType::I32);

    // Allocate memory for the string data
    body.i32_const(string_len)
        .call(env.get(RuntimeFunction::Malloc)) // Returns data_ptr
        .local_set(data_ptr_local); // Store in local and pop from stack

    // Get memory reference
    let memory_id = get_memory_id(module)?;

    // Write each byte of the string to the allocated memory
    for (offset, &byte) in string_bytes.iter().enumerate() {
        body.local_get(data_ptr_local);
        body.i32_const(byte as i32);
        body.store(
            memory_id,
            walrus::ir::StoreKind::I32_8 { atomic: false },
            walrus::ir::MemArg {
                align: 1,
                offset: offset as u64,
            },
        );
    }

    // Call cel_create_string(data_ptr, len)
    body.local_get(data_ptr_local); // Load data_ptr
    body.i32_const(string_len); // Load length
    body.call(env.get(RuntimeFunction::CreateString)); // Returns *mut CelValue

    let cel_val_local = module.locals.add(ValType::I32);
    body.local_set(cel_val_local);

    Ok(cel_val_local)
}

/// Compile a bytes literal, returning the LocalId of the resulting CelValue pointer.
fn compile_bytes_literal(
    bytes: &[u8],
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    module: &mut walrus::Module,
) -> Result<LocalId, anyhow::Error> {
    let bytes_len = bytes.len() as i32;

    // Create a local to store the bytes data pointer
    let data_ptr_local = module.locals.add(ValType::I32);

    // Allocate memory for the bytes data
    body.i32_const(bytes_len)
        .call(env.get(RuntimeFunction::Malloc)) // Returns data_ptr
        .local_set(data_ptr_local); // Store in local and pop from stack

    // Get memory reference
    let memory_id = get_memory_id(module)?;

    // Write each byte to the allocated memory
    for (offset, &byte) in bytes.iter().enumerate() {
        body.local_get(data_ptr_local);
        body.i32_const(byte as i32);
        body.store(
            memory_id,
            walrus::ir::StoreKind::I32_8 { atomic: false },
            walrus::ir::MemArg {
                align: 1,
                offset: offset as u64,
            },
        );
    }

    // Call cel_create_bytes(data_ptr, len)
    body.local_get(data_ptr_local); // Load data_ptr
    body.i32_const(bytes_len); // Load length
    body.call(env.get(RuntimeFunction::CreateBytes)); // Returns *mut CelValue

    let cel_val_local = module.locals.add(ValType::I32);
    body.local_set(cel_val_local);

    Ok(cel_val_local)
}
