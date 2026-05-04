use std::{env, fs, path::PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // Search order:
    //   1. $CARGO_MANIFEST_DIR/runtime.wasm — present when building from a published crate
    //      (placed there by `make publish-prep` before `cargo publish`)
    //   2. workspace target directory — present during normal workspace development
    let bundled = manifest_dir.join("runtime.wasm");
    let workspace = manifest_dir.join("../target/wasm32-unknown-unknown/release/runtime.wasm");

    let source = if bundled.exists() {
        println!("cargo:rerun-if-changed=runtime.wasm");
        bundled
    } else if workspace.exists() {
        println!("cargo:rerun-if-changed={}", workspace.display());
        workspace
    } else {
        panic!(
            "runtime.wasm not found.\n\
             In a workspace, run `make runtime` first.\n\
             When publishing, run `make publish-prep` before `cargo publish`."
        );
    };

    fs::copy(&source, out_dir.join("runtime.wasm"))
        .unwrap_or_else(|e| panic!("failed to copy runtime.wasm from {}: {e}", source.display()));
}
