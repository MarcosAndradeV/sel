use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=tests/ffi_test_structs.c");
    println!("cargo:rerun-if-changed=tests/ffi_test_structs.so");
    let status = Command::new("gcc")
        .args([
            "-shared",
            "-o",
            "./tests/libffi_test_structs.so",
            "-fPIC",
            "tests/ffi_test_structs.c",
        ])
        .status();
    if let Err(e) = status {
        eprintln!("Failed to compile ffi_test_structs.c: {}", e);
    }
}
