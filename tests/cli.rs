use std::process::Command;

#[test]
fn version_option_reports_the_crate_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_tesseract-to-markdown"))
        .arg("--version")
        .output()
        .expect("CLI should run");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("version output should be UTF-8"),
        format!("tesseract-to-markdown {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(output.stderr, [] as [u8; 0]);
}
