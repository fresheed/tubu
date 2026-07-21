// Shared by build.rs and src/bin/build_so.rs via `include!`, since build.rs
// isn't part of the crate's dependency graph and can't `use` its modules.

use std::process::Command;

const ERR_MSG: &str = "failed to compile interceptor library!";
const SRC_PATH: &str = "intercept/av_log_intercept.c";
const OUT_PATH: &str = "intercept/av_log_intercept.so";

fn compile_interceptor() {
    let status = Command::new("gcc")
        .args([
            "-Wall", "-Wextra", "-Werror",
            "-fPIC", "-shared",
            "-O2", "-g",
            "-o", OUT_PATH,
            SRC_PATH,
        ])
        .status()
        .expect(ERR_MSG);
    if !status.success() {
        panic!("{}", ERR_MSG);
    }
}
