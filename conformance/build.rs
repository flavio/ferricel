// Build script for conformance tests - generates proto descriptor
use std::{env, io::Result, path::PathBuf};

fn main() -> Result<()> {
    // Configure prost to generate Rust code from CEL protobuf definitions
    let proto_dir = "../cel-spec/proto";

    // Get the OUT_DIR where build script outputs go
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let descriptor_path = out_dir.join("conformance_test_protos.bin");

    prost_build::Config::new()
        .bytes(["."])
        .file_descriptor_set_path(&descriptor_path)
        .type_attribute(".", "#[allow(dead_code)]")
        .compile_protos(
            &[
                "cel/expr/value.proto",
                "cel/expr/checked.proto",
                "cel/expr/eval.proto",
                "cel/expr/conformance/test/simple.proto",
                "cel/expr/conformance/proto2/test_all_types.proto",
                "cel/expr/conformance/proto3/test_all_types.proto",
            ],
            &[proto_dir],
        )?;

    // Tell cargo to re-run build script if proto files change
    println!("cargo:rerun-if-changed=../cel-spec/proto");

    // Export the path so tests can find it
    println!(
        "cargo:rustc-env=PROTO_DESCRIPTOR_PATH={}",
        descriptor_path.display()
    );

    Ok(())
}
