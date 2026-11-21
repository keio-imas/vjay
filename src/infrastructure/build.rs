fn main() {
    for file in [
        "movie.hpp",
        "movie.mm",
        "movie_decode.mm",
        "movie_gpu.mm",
        "movie_samples.mm",
    ] {
        println!("cargo:rerun-if-changed=../../native/macos/{file}");
    }
    println!("cargo:rerun-if-changed=../../native/macos/capture.mm");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .cpp(true)
            .file("../../native/macos/capture.mm")
            .files([
                "../../native/macos/movie.mm",
                "../../native/macos/movie_decode.mm",
                "../../native/macos/movie_gpu.mm",
                "../../native/macos/movie_samples.mm",
            ])
            .flag("-std=c++17")
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .flag("-mmacosx-version-min=11.0")
            .compile("vjay_capture");
        for name in [
            "AVFoundation",
            "CoreMedia",
            "CoreVideo",
            "VideoToolbox",
            "CoreGraphics",
            "Metal",
            "MetalPerformanceShaders",
            "Foundation",
        ] {
            println!("cargo:rustc-link-lib=framework={name}");
        }
    }
}
