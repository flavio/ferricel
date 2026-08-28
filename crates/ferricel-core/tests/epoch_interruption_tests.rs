// Integration tests for wasmtime epoch-based interruption support.
//
// These tests exercise `runtime::Builder::with_epoch_deadline` together with
// a custom `wasmtime::Engine` that has `Config::epoch_interruption(true)`
// set. They intentionally do not use the shared `compile_and_execute*`
// helpers in `common.rs` because those always build a default
// `wasmtime::Engine`.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use ferricel_core::{compiler, runtime};
use wasmtime::{Config, Engine as WasmEngine};

/// Build an engine with epoch interruption enabled, compile `cel_expr`, and
/// evaluate it with the given `bindings_json` and `epoch_deadline`.
fn eval_with_epoch_interruption(
    cel_expr: &str,
    bindings_json: Option<&str>,
    epoch_deadline: Option<u64>,
) -> Result<String, anyhow::Error> {
    let mut config = Config::new();
    config.epoch_interruption(true);
    let wasm_engine = WasmEngine::new(&config)?;

    let wasm = compiler::Builder::new().build().compile(cel_expr)?;

    let mut builder = runtime::Builder::new()
        .with_engine(wasm_engine)
        .with_wasm(wasm);
    if let Some(deadline) = epoch_deadline {
        builder = builder.with_epoch_deadline(deadline);
    }

    builder.build()?.eval(bindings_json)
}

#[test]
fn deadline_propagation_allows_evaluation_to_succeed() {
    // Engine has epoch interruption enabled but nothing ever calls
    // `increment_epoch()`, so as long as the deadline is wired through to
    // the Store, the evaluation must succeed. Without the
    // `set_epoch_deadline` wiring in `eval_raw`, this traps immediately per
    // wasmtime semantics (interruption-enabled engine + no store deadline).
    let result = eval_with_epoch_interruption("1 + 1", None, Some(1));

    assert!(
        result.is_ok(),
        "expected evaluation to succeed with an epoch deadline set, got: {:?}",
        result
    );
    assert_eq!(result.unwrap(), "2");
}

#[test]
fn no_deadline_with_interruption_enabled_engine_traps_immediately() {
    // Same interruption-enabled engine, but no deadline configured on the
    // Builder. This documents/locks in wasmtime's behavior: a Store with no
    // configured epoch deadline on an interruption-enabled engine traps on
    // its first epoch-checking instruction.
    let result = eval_with_epoch_interruption("1 + 1", None, None);

    assert!(
        result.is_err(),
        "expected evaluation to fail (immediate trap) with no epoch deadline set"
    );
}

#[test]
fn actual_interruption_stops_slow_evaluation() {
    // Build a large bindings list and a nested comprehension so evaluation
    // is slow enough (O(n^2)) to be reliably interrupted, without relying on
    // any unbounded-loop construct (CEL has none).
    const N: usize = 2000;
    let list: Vec<i64> = (0..N as i64).collect();
    let bindings = serde_json::json!({ "input": list }).to_string();

    let mut config = Config::new();
    config.epoch_interruption(true);
    let wasm_engine = WasmEngine::new(&config).expect("failed to create wasmtime engine");

    let wasm = compiler::Builder::new()
        .build()
        .compile("input.all(x, input.all(y, x + y >= 0))")
        .expect("failed to compile expression");

    let engine = runtime::Builder::new()
        .with_engine(wasm_engine.clone())
        .with_epoch_deadline(1)
        .with_wasm(wasm)
        .build()
        .expect("failed to build engine");

    let stop = Arc::new(AtomicBool::new(false));
    let ticker_stop = stop.clone();
    let ticker_engine = wasm_engine.clone();
    let ticker = std::thread::spawn(move || {
        while !ticker_stop.load(Ordering::Relaxed) {
            ticker_engine.increment_epoch();
            std::thread::sleep(Duration::from_millis(5));
        }
    });

    let start = Instant::now();
    let result = engine.eval(Some(&bindings));
    let elapsed = start.elapsed();

    stop.store(true, Ordering::Relaxed);
    ticker.join().expect("ticker thread panicked");

    assert!(
        result.is_err(),
        "expected the slow evaluation to be interrupted and return Err"
    );
    assert!(
        elapsed < Duration::from_millis(500),
        "expected interruption to stop evaluation quickly (uninterrupted runtime is ~1.8s), took {:?}",
        elapsed
    );
}

#[test]
fn deadline_is_a_noop_without_epoch_interruption() {
    // Default engine (no epoch_interruption configured). Setting a deadline
    // on a Store whose engine lacks epoch interruption support is a no-op:
    // evaluation should succeed normally.
    let wasm = compiler::Builder::new()
        .build()
        .compile("1 + 1")
        .expect("failed to compile expression");

    let result = runtime::Builder::new()
        .with_epoch_deadline(5)
        .with_wasm(wasm)
        .build()
        .expect("failed to build engine")
        .eval(None);

    assert!(
        result.is_ok(),
        "expected evaluation to succeed when epoch interruption is not enabled, got: {:?}",
        result
    );
    assert_eq!(result.unwrap(), "2");
}
