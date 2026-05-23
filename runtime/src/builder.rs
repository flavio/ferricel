//! Runtime support for fluent builder chain extensions.
//!
//! The compiler lowers each builder step (Entry, Chain, Terminal) into calls to
//! [`cel_builder_step`].  This function maintains a `CelValue::Object` map that
//! accumulates the chain's state, tagged with a `"__type__"` discriminator so
//! the host can identify which builder type it is dealing with.
//!
//! Terminal steps call [`cel_call_extension`] directly (via the existing
//! `ExtCall1` / `ExtCall2` infrastructure); this module is only needed for the
//! intermediate accumulation steps.

use std::collections::HashMap;

use crate::{
    error::abort_with_error,
    types::{CelMapKey, CelValue},
};

/// Read a UTF-8 string from Wasm linear memory.
///
/// # Safety
/// `ptr` must point to `len` valid, initialised bytes in Wasm linear memory.
#[inline]
unsafe fn read_str<'a>(ptr: i32, len: i32) -> &'a str {
    let slice = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
    std::str::from_utf8(slice).unwrap_or_else(|_| abort_with_error("invalid UTF-8 in builder key"))
}

/// Produce or update a builder state map for one step in a fluent chain.
///
/// # Arguments
/// - `receiver`      — existing state map (`*mut CelValue`), or `0` (null) to start fresh
/// - `type_tag_ptr / type_tag_len` — UTF-8 bytes of the `"__type__"` tag for the output map
/// - `key_ptr / key_len`           — UTF-8 bytes of the field key to set or append to
/// - `value`         — the `*mut CelValue` to store under `key`
/// - `accumulate`    — `0` = overwrite (or insert), `1` = append to an existing array
///
/// # Returns
/// A heap-allocated `*mut CelValue` holding the updated `CelValue::Object`.
///
/// # Safety
/// - `value` must be a valid, non-null `*mut CelValue`.
/// - `type_tag_ptr`, `key_ptr` must point to valid UTF-8 memory.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_builder_step(
    receiver: *mut CelValue,
    type_tag_ptr: i32,
    type_tag_len: i32,
    key_ptr: i32,
    key_len: i32,
    value: *mut CelValue,
    accumulate: i32,
) -> *mut CelValue {
    if value.is_null() {
        abort_with_error("cel_builder_step: value pointer is null");
    }

    // Clone or create the underlying map.
    let mut map: HashMap<CelMapKey, CelValue> = if receiver.is_null() {
        HashMap::new()
    } else {
        match &*receiver {
            CelValue::Object(m) => m.clone(),
            other => {
                // Receiver is not a map — this should not happen in a well-formed chain.
                let _ = other;
                abort_with_error("cel_builder_step: receiver is not an Object map");
            }
        }
    };

    // Update the __type__ tag.
    let type_tag = read_str(type_tag_ptr, type_tag_len).to_string();
    map.insert(
        CelMapKey::String("__type__".to_string()),
        CelValue::String(type_tag),
    );

    // Set or append the new value.
    let key = read_str(key_ptr, key_len).to_string();
    let val = (*value).clone();

    if accumulate != 0 {
        // Append to an existing array, or create a new one.
        let array_key = CelMapKey::String(key.clone());
        let new_entry = match map.remove(&array_key) {
            Some(CelValue::Array(mut arr)) => {
                arr.push(val);
                CelValue::Array(arr)
            }
            Some(existing) => CelValue::Array(vec![existing, val]),
            None => CelValue::Array(vec![val]),
        };
        map.insert(CelMapKey::String(key), new_entry);
    } else {
        map.insert(CelMapKey::String(key), val);
    }

    Box::into_raw(Box::new(CelValue::Object(map)))
}
