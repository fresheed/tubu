// See build_common.rs for the shared compilation logic (also used by build.rs).
include!(concat!(env!("CARGO_MANIFEST_DIR"), "/build_common.rs"));

fn main() {
    compile_interceptor();
}
