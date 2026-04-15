use crate::common::*;
use rstest::rstest;

// --- sets.contains ---

#[rstest]
// Empty lists
#[case::both_empty("sets.contains([], [])", 1)]
#[case::empty_sublist("sets.contains([1], [])", 1)]
// Basic containment
#[case::single("sets.contains([1], [1])", 1)]
#[case::dup_in_sublist("sets.contains([1], [1, 1])", 1)]
#[case::dup_in_list("sets.contains([1, 1], [1])", 1)]
#[case::reordered("sets.contains([2, 1], [1])", 1)]
#[case::subset("sets.contains([1, 2, 3, 4], [2, 3])", 1)]
// Cross-type numeric equality
#[case::int_double("sets.contains([1], [1.0, 1])", 1)]
#[case::int_uint_double("sets.contains([1, 2], [2u, 2.0])", 1)]
#[case::uint_in_int_list("sets.contains([1, 2u], [2, 2.0])", 1)]
#[case::mixed_numeric("sets.contains([1, 2.0, 3u], [1.0, 2u, 3])", 1)]
// Nested lists
#[case::nested("sets.contains([[1], [2, 3]], [[2, 3.0]])", 1)]
// Negative cases
#[case::not_found("!sets.contains([1], [2])", 1)]
#[case::partial_miss("!sets.contains([1], [1, 2])", 1)]
#[case::type_mismatch("!sets.contains([1], [\"1\", 1])", 1)]
#[case::close_but_no("!sets.contains([1], [1.1, 1u])", 1)]
fn test_sets_contains(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// --- sets.equivalent ---

#[rstest]
// Empty lists
#[case::both_empty("sets.equivalent([], [])", 1)]
// Basic equivalence
#[case::single("sets.equivalent([1], [1])", 1)]
#[case::dup_in_second("sets.equivalent([1], [1, 1])", 1)]
#[case::dup_in_first("sets.equivalent([1, 1], [1])", 1)]
// Cross-type numeric equality
#[case::int_uint_double("sets.equivalent([1], [1u, 1.0])", 1)]
#[case::reordered_mixed("sets.equivalent([1, 2, 3], [3u, 2.0, 1])", 1)]
// Nested lists
#[case::nested("sets.equivalent([[1.0], [2, 3]], [[1], [2, 3.0]])", 1)]
// Negative cases
#[case::superset_not_equiv("!sets.equivalent([2, 1], [1])", 1)]
#[case::subset_not_equiv("!sets.equivalent([1], [1, 2])", 1)]
#[case::numeric_mismatch_a("!sets.equivalent([1, 2], [2u, 2, 2.0])", 1)]
#[case::numeric_mismatch_b("!sets.equivalent([1, 2], [1u, 2, 2.3])", 1)]
fn test_sets_equivalent(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// --- sets.intersects ---

#[rstest]
// Basic intersection
#[case::single("sets.intersects([1], [1])", 1)]
#[case::dup_in_second("sets.intersects([1], [1, 1])", 1)]
#[case::dup_in_first("sets.intersects([1, 1], [1])", 1)]
#[case::reordered("sets.intersects([2, 1], [1])", 1)]
#[case::partial("sets.intersects([1], [1, 2])", 1)]
// Cross-type numeric equality
#[case::int_double("sets.intersects([1], [1.0, 2])", 1)]
#[case::mixed_numeric_a("sets.intersects([1, 2], [2u, 2, 2.0])", 1)]
#[case::mixed_numeric_b("sets.intersects([1, 2], [1u, 2, 2.3])", 1)]
// Nested lists
#[case::nested("sets.intersects([[1], [2, 3]], [[1, 2], [2, 3.0]])", 1)]
// Negative cases
#[case::both_empty("!sets.intersects([], [])", 1)]
#[case::empty_second("!sets.intersects([1], [])", 1)]
#[case::no_match("!sets.intersects([1], [2])", 1)]
#[case::type_mismatch("!sets.intersects([1], [\"1\", 2])", 1)]
#[case::close_but_no("!sets.intersects([1], [1.1, 2u])", 1)]
fn test_sets_intersects(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}
