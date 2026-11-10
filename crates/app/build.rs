const MAIN_THREAD_STACK: u32 = 8 * 1024 * 1024;

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    if target_os != "windows" {
        return;
    }

    let argument = if target_env == "msvc" {
        format!("/STACK:{MAIN_THREAD_STACK}")
    } else {
        format!("-Wl,--stack,{MAIN_THREAD_STACK}")
    };
    println!("cargo:rustc-link-arg-bins={argument}");
}
