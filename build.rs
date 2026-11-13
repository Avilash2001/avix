fn main() {
    // Embed a Windows application manifest (DPI awareness, modern common controls,
    // supported OS list, asInvoker). Avix elevates itself at runtime when it needs to,
    // so it can still run in a limited "no admin" mode.
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest::embed_manifest(embed_manifest::new_manifest("Avix.Optimizer"))
            .expect("unable to embed manifest");
    }

    // GNU toolchain only: rustup's minimal MinGW runtime ships no libshlwapi.a, but the
    // `webbrowser` crate (pulled in by eframe) links `-lshlwapi` for one function.
    // An empty archive satisfies the flag; the symbol itself resolves from the
    // windows-targets import library that is already on the link line.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
        let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("stub-libs");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("libshlwapi.a"), b"!<arch>\n").unwrap();
        println!("cargo:rustc-link-search=native={}", out.display());
    }
    println!("cargo:rerun-if-changed=build.rs");
}
