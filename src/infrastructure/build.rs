fn main() {
    println!("cargo:rerun-if-changed=../../native/macos/capture.mm");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .cpp(true)
            .file("../../native/macos/capture.mm")
            .flag("-std=c++17")
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .flag("-mmacosx-version-min=11.0")
            .compile("vjay_capture");
        for name in [
            "AVFoundation",
            "CoreMedia",
            "CoreVideo",
            "Metal",
            "MetalPerformanceShaders",
            "Foundation",
        ] {
            println!("cargo:rustc-link-lib=framework={name}");
        }
    }
}
