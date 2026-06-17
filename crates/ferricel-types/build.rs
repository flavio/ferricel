use std::io::Result;

fn main() -> Result<()> {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let proto_dir = manifest_dir.join("proto");

    prost_build::Config::new().bytes(["."]).compile_protos(
        &[
            proto_dir.join("cel/expr/value.proto"),
            proto_dir.join("bindings.proto"),
        ],
        &[&proto_dir],
    )?;

    println!("cargo:rerun-if-changed=proto");

    Ok(())
}
