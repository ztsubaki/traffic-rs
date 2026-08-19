use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=bridge/libsumo-bridge.cpp");
    println!("cargo:rerun-if-changed=bridge/libsumo-bridge.h");
    println!("cargo:rerun-if-env-changed=SUMO_DIR");

    match env::var_os("SUMO_DIR") {
        Some(value) => {
            let sumo_path = PathBuf::from(value);
            let include = sumo_path.join("./include");
            let lib_dir = sumo_path.join("./lib");

            cc::Build::new()
                .cpp(true)
                .std("c++17")
                .include(&include)
                .file("bridge/libsumo-bridge.cpp")
                .compile("libsumo-bridge");
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
        }
        None => {
            cc::Build::new()
                .cpp(true)
                .std("c++17")
                .file("bridge/libsumo-bridge.cpp")
                .compile("libsumo-bridge");
        }
    }

    println!("cargo:rustc-link-lib=dylib=sumocpp");
    println!("cargo:rustc-link-lib=dylib=stdc++");
}
