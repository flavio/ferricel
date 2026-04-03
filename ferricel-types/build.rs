use std::io::Result;

fn main() -> Result<()> {
    let proto_dir = "proto";

    prost_build::Config::new()
        .bytes(["."])
        .compile_protos(&["cel/expr/value.proto", "bindings.proto"], &[proto_dir])?;

    println!("cargo:rerun-if-changed=proto");

    Ok(())
}
