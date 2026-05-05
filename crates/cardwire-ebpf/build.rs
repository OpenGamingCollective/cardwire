use std::{env, path::PathBuf, process::Command};

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = PathBuf::from(out_dir).join("bpf.o");
    let source_path = "src/bpf.c";
    let status = Command::new("clang")
        .args([
            "-O3",
            "-g",
            "-target",
            "bpf",
            "-c",
            source_path,
            "-o",
            out_path.to_str().unwrap(),
        ])
        .env("NIX_HARDENING_ENABLE", "")
        .status()
        .expect("Failed to execute clang");

    if !status.success() {
        panic!("Failed to compile BPF program");
    }
}
