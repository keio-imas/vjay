fn main() {
    println!("cargo:rerun-if-changed=native/macos/Info.plist");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let plist = std::path::PathBuf::from(
            std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"),
        )
        .join("native/macos/Info.plist");
        println!(
            "cargo:rustc-link-arg=-Wl,-sectcreate,__TEXT,__info_plist,{}",
            plist.display()
        );
    }
}
