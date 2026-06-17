use rstest::rstest;

use crate::common::*;

#[rstest]
#[case("[1, 2, 3].isSorted()", true)]
#[case("[1, 1, 2].isSorted()", true)]
#[case("[3, 1, 2].isSorted()", false)]
#[case("[].isSorted()", true)]
#[case("[42].isSorted()", true)]
fn test_k8s_list_is_sorted(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("[].sum()", 0)]
#[case("[1, 2, 3].sum()", 6)]
#[case(r#"[42].sum()"#, 42)]
fn test_k8s_list_sum_int(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[test]
fn test_k8s_list_sum_double() {
    let result = compile_and_execute_double("[1.5, 2.5].sum()").expect("compile/execute failed");
    assert!((result - 4.0).abs() < 1e-9, "Expected 4.0, got {}", result);
}

#[rstest]
#[case("[3, 1, 2].min()", 1)]
#[case("[7].min()", 7)]
#[case("[-5, 0, 5].min()", -5)]
fn test_k8s_list_min(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("[3, 1, 2].max()", 3)]
#[case("[7].max()", 7)]
#[case("[-5, 0, 5].max()", 5)]
fn test_k8s_list_max(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("[1, 2, 2, 3].indexOf(2)", 1)]
#[case("[1, 2, 3].indexOf(99)", -1)]
#[case("[].indexOf(1)", -1)]
fn test_k8s_list_index_of(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("[1, 2, 2, 3].lastIndexOf(2)", 2)]
#[case("[1, 2, 3].lastIndexOf(99)", -1)]
#[case("[].lastIndexOf(1)", -1)]
fn test_k8s_list_last_index_of(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}
