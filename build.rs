#![allow(clippy::unwrap_used)]
use std::process::Command;
use std::str::from_utf8;

fn generate_version() {
    let version = Command::new("git")
        .args(["describe", "--tags", "--always"])
        .output()
        .unwrap();
    let mut version = from_utf8(version.stdout.as_slice())
        .unwrap()
        .trim()
        .to_string();
    if version.is_empty() {
        version = "0.0.0".into();
    }
    println!("cargo:rustc-env=VERSION={}", version);
    println!("cargo:rustc-rerun-if-changed=.git/HEAD");
    println!("cargo:rustc-rerun-if-changed=build.rs");
}

fn main() {
    generate_version();
}
