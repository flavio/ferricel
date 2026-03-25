use ferricel_types::functions::RuntimeFunction;
use walrus::{InstrSeqBuilder, LocalId, ValType};

use super::context::CompilerEnv;

/// Returns the single memory id from the module, or an error if none exists.
pub fn get_memory_id(module: &walrus::Module) -> Result<walrus::MemoryId, anyhow::Error> {
    module
        .memories
        .iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No memory found"))
        .map(|m| m.id())
}

/// Helper function to compile a string literal into a CelValue and store it in a local.
/// Returns the LocalId containing the pointer to the CelValue::String.
///
/// This is used for struct field names and type names to avoid code duplication.
pub fn compile_string_to_local(
    s: &str,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    module: &mut walrus::Module,
) -> Result<LocalId, anyhow::Error> {
    let bytes = s.as_bytes();
    let len = bytes.len() as i32;

    // Allocate memory for the string data
    let data_ptr_local = module.locals.add(ValType::I32);
    body.i32_const(len)
        .call(env.get(RuntimeFunction::Malloc))
        .local_set(data_ptr_local);

    // Get memory reference
    let memory_id = get_memory_id(module)?;

    // Write each byte of the string to the allocated memory
    for (offset, &byte) in bytes.iter().enumerate() {
        body.local_get(data_ptr_local);
        body.i32_const(byte as i32);
        body.store(
            memory_id,
            walrus::ir::StoreKind::I32_8 { atomic: false },
            walrus::ir::MemArg {
                align: 1,
                offset: offset as u32,
            },
        );
    }

    // Call cel_create_string(data_ptr, len)
    body.local_get(data_ptr_local);
    body.i32_const(len);
    body.call(env.get(RuntimeFunction::CreateString));

    // Store the resulting CelValue pointer in a local
    let result_local = module.locals.add(ValType::I32);
    body.local_set(result_local);

    // Free the temporary raw bytes buffer — cel_create_string already copied
    // the data into an owned String, so the original allocation is no longer needed
    body.local_get(data_ptr_local);
    body.i32_const(len);
    body.call(env.get(RuntimeFunction::Free));

    Ok(result_local)
}

/// Emit instructions to write a compile-time string constant into WASM memory
/// and leave `(ptr: i32, len: i32)` on the stack.
///
/// When `s` is empty, pushes `(0i32, 0i32)` without any allocation.
pub fn emit_string_const(
    s: &str,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    memory_id: walrus::MemoryId,
    module: &mut walrus::Module,
) {
    let bytes = s.as_bytes();
    let len = bytes.len() as i32;
    if len == 0 {
        body.i32_const(0);
        body.i32_const(0);
        return;
    }
    let ptr_local = module.locals.add(ValType::I32);
    body.i32_const(len)
        .call(env.get(RuntimeFunction::Malloc))
        .local_set(ptr_local);
    for (offset, &byte) in bytes.iter().enumerate() {
        body.local_get(ptr_local);
        body.i32_const(byte as i32);
        body.store(
            memory_id,
            walrus::ir::StoreKind::I32_8 { atomic: false },
            walrus::ir::MemArg {
                align: 1,
                offset: offset as u32,
            },
        );
    }
    body.local_get(ptr_local);
    body.i32_const(len);
}
