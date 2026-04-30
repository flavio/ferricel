//! Extension function support for the WASM guest runtime.
//!
//! Provides the host import declaration and fixed-arity wrappers that compiled
//! CEL programs use to call host-provided extension functions.

use crate::error::read_ptr;
use crate::memory::cel_malloc;
use crate::serialization::encode_ptr_len;
use crate::types::CelValue;
use ferricel_types::extensions::ExtensionCallPayload;

// Host import: call a host-provided extension function.
//
// The packed i64 encodes the JSON request:
//   - low 32 bits  = pointer to JSON bytes in WASM memory
//   - high 32 bits = length of JSON bytes
//
// Returns a packed i64 with the JSON response in the same format.
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn cel_call_extension(packed: i64) -> i64;
}

/// Internal helper that serializes the extension call, invokes the host, and
/// deserializes the response into a heap-allocated `CelValue`.
///
/// # Safety
///
/// All pointers in `args` must be valid, non-null `*mut CelValue` pointers.
unsafe fn call_extension_impl(
    namespace: Option<&str>,
    function: &str,
    args: Vec<CelValue>,
) -> *mut CelValue {
    // Serialize each argument CelValue to a serde_json::Value.
    let json_args: Vec<serde_json::Value> = args
        .into_iter()
        .map(|val| serde_json::to_value(&val).expect("Failed to serialize CelValue to JSON"))
        .collect();

    // Build the wire-format payload.
    let payload = ExtensionCallPayload {
        namespace: namespace.map(|s| s.to_string()),
        function: function.to_string(),
        args: json_args,
    };

    // Serialize the payload to JSON bytes.
    let json_bytes =
        serde_json::to_vec(&payload).expect("Failed to serialize ExtensionCallPayload");
    let json_len = json_bytes.len();

    // Allocate WASM memory for the request JSON and copy the bytes in.
    let req_ptr = cel_malloc(json_len);
    unsafe {
        std::ptr::copy_nonoverlapping(json_bytes.as_ptr(), req_ptr, json_len);
    }

    // Pack (ptr, len) and call the host import.
    let req_packed = encode_ptr_len(req_ptr as i32, json_len as i32);
    let resp_packed = unsafe { cel_call_extension(req_packed) };

    // Unpack the response pointer and length.
    let resp_ptr = (resp_packed & 0xFFFFFFFF) as u32 as usize;
    let resp_len = (resp_packed >> 32) as u32 as usize;

    // Read the response JSON from WASM memory.
    let resp_bytes: &[u8] = unsafe { std::slice::from_raw_parts(resp_ptr as *const u8, resp_len) };

    // Deserialize the response into a CelValue.
    let result: CelValue =
        serde_json::from_slice(resp_bytes).unwrap_or_else(|e| CelValue::Error(e.to_string()));

    Box::into_raw(Box::new(result))
}

// ---------------------------------------------------------------------------
// Fixed-arity exported wrapper functions
// ---------------------------------------------------------------------------

/// Call a host extension with 0 arguments.
///
/// # Safety
///
/// `ns_ptr`/`method_ptr` must point to valid UTF-8 bytes of the given lengths.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_ext_call_0(
    ns_ptr: *const u8,
    ns_len: i32,
    method_ptr: *const u8,
    method_len: i32,
) -> *mut CelValue {
    let namespace = read_optional_str(ns_ptr, ns_len);
    let function = read_str(method_ptr, method_len);
    unsafe { call_extension_impl(namespace, function, vec![]) }
}

/// Call a host extension with 1 argument.
///
/// # Safety
///
/// All pointers must be valid for the described types/lengths.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_ext_call_1(
    ns_ptr: *const u8,
    ns_len: i32,
    method_ptr: *const u8,
    method_len: i32,
    arg0: *mut CelValue,
) -> *mut CelValue {
    let namespace = read_optional_str(ns_ptr, ns_len);
    let function = read_str(method_ptr, method_len);
    let a0 = unsafe { read_ptr(arg0) };
    unsafe { call_extension_impl(namespace, function, vec![a0]) }
}

/// Call a host extension with 2 arguments.
///
/// # Safety
///
/// All pointers must be valid for the described types/lengths.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_ext_call_2(
    ns_ptr: *const u8,
    ns_len: i32,
    method_ptr: *const u8,
    method_len: i32,
    arg0: *mut CelValue,
    arg1: *mut CelValue,
) -> *mut CelValue {
    let namespace = read_optional_str(ns_ptr, ns_len);
    let function = read_str(method_ptr, method_len);
    let a0 = unsafe { read_ptr(arg0) };
    let a1 = unsafe { read_ptr(arg1) };
    unsafe { call_extension_impl(namespace, function, vec![a0, a1]) }
}

/// Call a host extension with 3 arguments.
///
/// # Safety
///
/// All pointers must be valid for the described types/lengths.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_ext_call_3(
    ns_ptr: *const u8,
    ns_len: i32,
    method_ptr: *const u8,
    method_len: i32,
    arg0: *mut CelValue,
    arg1: *mut CelValue,
    arg2: *mut CelValue,
) -> *mut CelValue {
    let namespace = read_optional_str(ns_ptr, ns_len);
    let function = read_str(method_ptr, method_len);
    let a0 = unsafe { read_ptr(arg0) };
    let a1 = unsafe { read_ptr(arg1) };
    let a2 = unsafe { read_ptr(arg2) };
    unsafe { call_extension_impl(namespace, function, vec![a0, a1, a2]) }
}

/// Call a host extension with 4 arguments.
///
/// # Safety
///
/// All pointers must be valid for the described types/lengths.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_ext_call_4(
    ns_ptr: *const u8,
    ns_len: i32,
    method_ptr: *const u8,
    method_len: i32,
    arg0: *mut CelValue,
    arg1: *mut CelValue,
    arg2: *mut CelValue,
    arg3: *mut CelValue,
) -> *mut CelValue {
    let namespace = read_optional_str(ns_ptr, ns_len);
    let function = read_str(method_ptr, method_len);
    let a0 = unsafe { read_ptr(arg0) };
    let a1 = unsafe { read_ptr(arg1) };
    let a2 = unsafe { read_ptr(arg2) };
    let a3 = unsafe { read_ptr(arg3) };
    unsafe { call_extension_impl(namespace, function, vec![a0, a1, a2, a3]) }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Read a `&str` from a raw pointer + length without copying.
///
/// # Safety
///
/// `ptr` must point to `len` valid UTF-8 bytes that live at least as long as
/// the returned reference.
#[inline]
unsafe fn read_str<'a>(ptr: *const u8, len: i32) -> &'a str {
    let bytes: &'a [u8] = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    std::str::from_utf8(bytes).expect("Extension function name is not valid UTF-8")
}

/// Read a `&str` from a raw pointer + length, returning `None` when `len == 0`.
///
/// # Safety
///
/// Same as [`read_str`]. When `len > 0`, `ptr` must point to `len` valid UTF-8 bytes.
#[inline]
unsafe fn read_optional_str<'a>(ptr: *const u8, len: i32) -> Option<&'a str> {
    if len == 0 {
        None
    } else {
        Some(unsafe { read_str(ptr, len) })
    }
}
