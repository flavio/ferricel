use std::io::Result;

fn main() -> Result<()> {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let proto_dir = manifest_dir.join("proto");

    // Compile .proto files to a FileDescriptorSet using protox (pure Rust,
    // no `protoc` binary required), then generate Rust code with prost-build.
    let file_descriptors = protox::compile(
        [
            proto_dir.join("cel/expr/value.proto"),
            proto_dir.join("bindings.proto"),
        ],
        [&proto_dir],
    )
    .map_err(|e| std::io::Error::other(e.to_string()))?;

    prost_build::Config::new()
        .bytes(["."])
        .compile_fds(file_descriptors)?;

    println!("cargo:rerun-if-changed=proto");

    Ok(())
}
