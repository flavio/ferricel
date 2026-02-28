use std::io::Result;

fn main() -> Result<()> {
    // Configure prost to generate Rust code from CEL protobuf definitions
    let proto_dir = "../cel-spec/proto";

    prost_build::Config::new().bytes(["."]).compile_protos(
        &[
            "cel/expr/value.proto",
            "cel/expr/checked.proto",
            "cel/expr/eval.proto",
            "cel/expr/conformance/test/simple.proto",
        ],
        &[proto_dir],
    )?;

    // Tell cargo to re-run build script if proto files change
    println!("cargo:rerun-if-changed=../cel-spec/proto");

    Ok(())
}
