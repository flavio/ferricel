// Generated protobuf types for the CEL conformance tests.
// The `include!` macros pull in code emitted by prost-build at compile time.

pub mod cel {
    pub mod expr {
        include!(concat!(env!("OUT_DIR"), "/cel.expr.rs"));

        pub mod conformance {
            pub mod test {
                include!(concat!(env!("OUT_DIR"), "/cel.expr.conformance.test.rs"));
            }
        }
    }
}

pub use cel::expr::conformance::test::{SimpleTest, SimpleTestFile};
pub use cel::expr::{ExprValue, Value};
