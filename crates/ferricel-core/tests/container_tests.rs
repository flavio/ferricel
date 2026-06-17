// Integration tests for container name resolution.

use crate::common::*;

// ============================================================
// Container Resolution Tests
// ============================================================

#[test]
fn test_container_no_schema_no_container() {
    // Without schema or container, unqualified names should still work (treated as arbitrary structs)
    let result = compile_with_container("MyType{field: 42}", None, None);
    assert!(
        result.is_ok(),
        "Should compile unqualified struct name without schema"
    );
}

#[test]
fn test_container_with_container_but_no_schema() {
    // With container but no schema, resolution should fall back to using the name as-is
    // This is a graceful degradation case
    let result = compile_with_container("MyType{field: 42}", Some("com.example"), None);
    assert!(
        result.is_ok(),
        "Should compile with container but no schema (graceful degradation)"
    );
}

// Note: More comprehensive tests with proto descriptors would require building
// the proto files first with protoc. For now, these tests verify the basic
// container resolution logic compiles correctly.
