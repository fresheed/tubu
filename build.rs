use std::process::Command;

const ERR_MSG: &str = "failed to compile interceptor library!";
const SRC_PATH: &str = "intercept/av_log_intercept.c";
const OUT_PATH: &str = "intercept/av_log_intercept.so";

fn main() {
    // Re-run this build script if the C source changes
    println!("cargo:rerun-if-changed={}", SRC_PATH);
    let mut cmd = Command::new("gcc");
    cmd.args(&[
        "-Wall", "-Wextra", "-Werror",
        "-fPIC", "-shared",
        "-O2", "-g",
        "-o", OUT_PATH,
        SRC_PATH,
    ]);
    let status = cmd.status().expect(ERR_MSG);
    if !status.success() { panic!("{}", ERR_MSG); }
}