//! SkyLight is where macOS keeps its Spaces, and it ships only as a private
//! framework, so the linker needs that directory on its search path.
fn main() {
    println!("cargo:rustc-link-search=framework=/System/Library/PrivateFrameworks");
    println!("cargo:rustc-link-lib=framework=SkyLight");
    println!("cargo:rerun-if-changed=build.rs");
}
