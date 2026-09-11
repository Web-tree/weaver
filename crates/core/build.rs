fn main() {
    // The target triple is only known to cargo, but self-update needs it to
    // pick the right release asset at runtime.
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=WVR_BUILD_TARGET={target}");
    println!("cargo:rerun-if-changed=build.rs");
}
