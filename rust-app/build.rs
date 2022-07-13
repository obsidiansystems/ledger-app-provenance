use std::process;
use std::path::Path;
use std::env;

fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=script.ld");

    let cosmos = env::var("COSMOS_SDK").unwrap();

    let temp_dir = tempfile::Builder::new()
        .prefix("buf-out")
        .tempdir()?;
    let buf_out_file = temp_dir.path().join("buf_out.bin");

    let output = process::Command::new(&"buf")
        .arg("build")
        .arg(cosmos)
        .arg("--type=cosmos.tx.v1beta1.Tx")
        .arg(format!("--output={}", buf_out_file.display()))
        .output()?;

    assert!(output.status.success(), "buf command returned non success status {}\nstderr:\n{}", output.status, String::from_utf8_lossy(&output.stderr));

    proto_gen::generate::generate_rust_code(
        &buf_out_file,
        Path::new("proto")
    );
    Ok(())
}
