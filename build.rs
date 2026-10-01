fn main() {
    // The bounded recursive parser needs the same headroom as Linux. MSVC's
    // default executable stack is only 1 MiB and can overflow in debug builds
    // before the syntax-depth diagnostic is reached.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        for binary in ["rewind", "rewindc"] {
            println!("cargo:rustc-link-arg-bin={binary}=/STACK:8388608");
        }
    }
}
