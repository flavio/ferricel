use rstest::rstest;

use crate::common::*;

#[rstest]
#[case::hello(r#"base64.encode(b'hello')"#, "aGVsbG8=")]
#[case::empty(r#"base64.encode(b'')"#, "")]
#[case::hello_world(r#"base64.encode(b'Hello World!')"#, "SGVsbG8gV29ybGQh")]
fn test_base64_encode(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}

#[rstest]
#[case::padded(r#"base64.decode('aGVsbG8=') == b'hello'"#, true)]
#[case::unpadded(r#"base64.decode('aGVsbG8') == b'hello'"#, true)]
#[case::empty(r#"base64.decode('') == b''"#, true)]
#[case::hello_world(r#"base64.decode('SGVsbG8gV29ybGQh') == b'Hello World!'"#, true)]
fn test_base64_decode(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[test]
fn test_base64_roundtrip() {
    // base64.decode(base64.encode(b'hello')) == b'hello'
    let result = compile_and_execute_bool(r#"base64.decode(base64.encode(b'hello')) == b'hello'"#)
        .expect("Failed to compile and execute");
    assert!(result, "Round-trip should return the original bytes");
}
