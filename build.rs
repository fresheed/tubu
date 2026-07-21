include!(concat!(env!("CARGO_MANIFEST_DIR"), "/build_common.rs"));

fn main() {
    // Re-run this build script if the C source changes, or if the compiled
    // library is missing (a nonexistent watched path always counts as changed)
    println!("cargo:rerun-if-changed={}", SRC_PATH);
    println!("cargo:rerun-if-changed={}", OUT_PATH);
    compile_interceptor();
}
