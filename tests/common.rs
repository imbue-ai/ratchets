//! Test utilities for ratchets integration tests

/// Result type alias for tests
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Extract Ok value or panic with context
#[macro_export]
macro_rules! assert_ok {
    ($expr:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => panic!("assertion failed: expected Ok, got Err({:?})", e),
        }
    };
    ($expr:expr, $msg:literal) => {
        match $expr {
            Ok(v) => v,
            Err(e) => panic!("{}: {:?}", $msg, e),
        }
    };
}

/// Extract Some value or panic with context
#[macro_export]
macro_rules! assert_some {
    ($expr:expr) => {
        match $expr {
            Some(v) => v,
            None => panic!("assertion failed: expected Some, got None"),
        }
    };
    ($expr:expr, $msg:literal) => {
        match $expr {
            Some(v) => v,
            None => panic!("{}: got None", $msg),
        }
    };
}
