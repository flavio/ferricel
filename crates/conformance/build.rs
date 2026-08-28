// Build script for conformance tests - generates proto descriptor
use std::{env, fs, io::Result, path::PathBuf};

use prost::Message;

fn main() -> Result<()> {
    // Directory containing the CEL protobuf definitions (git submodule)
    let proto_dir = "../../cel-spec/proto";

    // Get the OUT_DIR where build script outputs go
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let descriptor_path = out_dir.join("conformance_test_protos.bin");

    // Compile .proto files to a FileDescriptorSet using protox (pure Rust,
    // no `protoc` binary required).
    let file_descriptors = protox::compile(
        [
            "cel/expr/value.proto",
            "cel/expr/checked.proto",
            "cel/expr/eval.proto",
            "cel/expr/conformance/test/simple.proto",
            "cel/expr/conformance/proto2/test_all_types.proto",
            "cel/expr/conformance/proto3/test_all_types.proto",
        ],
        [proto_dir],
    )
    .map_err(|e| std::io::Error::other(e.to_string()))?;

    // Write the descriptor set so tests can load it at runtime
    fs::write(&descriptor_path, file_descriptors.encode_to_vec())?;

    // Generate Rust code from the descriptor set
    prost_build::Config::new()
        .bytes(["."])
        .type_attribute(".", "#[allow(dead_code)]")
        .compile_fds(file_descriptors)?;

    // Tell cargo to re-run build script if proto files change
    println!("cargo:rerun-if-changed=../../cel-spec/proto");

    // Export the path so tests can find it
    println!(
        "cargo:rustc-env=PROTO_DESCRIPTOR_PATH={}",
        descriptor_path.display()
    );

    Ok(())
}
