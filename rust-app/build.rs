use std::env;
use std::path::Path;
use std::process;
use tempfile;

fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=script.ld");
    let profile = std::env::var("PROFILE").unwrap();
    let debug_print = std::env::var("CARGO_FEATURE_SPECULOS").is_ok();
    let extra_debug_print = std::env::var("CARGO_FEATURE_EXTRA_DEBUG").is_ok();
    let target = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let reloc_size = match (
        profile.as_str(),
        debug_print,
        extra_debug_print,
        target.as_str(),
    ) {
        ("release", false, false, "nanosplus") => 1312,
        ("release", false, false, "nanos") => 768,
        ("release", false, false, "nanox") => 1312,
        (_, _, true, _) => 1024 * 10,
        _ => 1024 * 7,
    };
    println!("cargo:rustc-link-arg=--defsym=_reloc_size={reloc_size}");

    // Cosmos repo path
    let cosmos = env::var("COSMOS_SDK").unwrap();

    // Temp dir for buf output
    let temp_dir = tempfile::Builder::new().prefix("buf-out").tempdir()?;
    let buf_out_file = temp_dir.path().join("buf_out.bin");

    // Generate a FileDescriptorSet binary file using buf on the cosmos-sdk repo
    // targeting the type cosmos.tx.v1.beta1.Tx
    let output = process::Command::new(&"buf")
        .arg("build")
        .arg(cosmos)
        .arg("--type=cosmos.tx.v1beta1.Tx")
        .arg("--type=cosmos.tx.v1beta1.SignDoc")
        .arg("--type=cosmos.bank.v1beta1.MsgSend")
        .arg("--type=cosmos.bank.v1beta1.MsgMultiSend")
        .arg("--type=cosmos.staking.v1beta1.MsgDelegate")
        .arg("--type=cosmos.staking.v1beta1.MsgUndelegate")
        .arg("--type=cosmos.staking.v1beta1.MsgBeginRedelegate")
        .arg("--type=cosmos.gov.v1beta1.MsgDeposit")
        .arg(format!("--output={}", buf_out_file.display()))
        .output()?;

    assert!(
        output.status.success(),
        "buf command returned non success status {}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    // Use the FileDescriptorSet binary file to generate rust code
    // under $OUT_DIR/proto
    proto_gen::generate::generate_rust_code(&buf_out_file, Path::new("proto"));
    Ok(())
}
