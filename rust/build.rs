extern crate gcc;
extern crate bindgen;

use std::env;
use std::path::PathBuf;

fn main() {

    // Compile the library
    gcc::Build::new()
        .file("reliable.c")
        .define("RELIABLE_ENABLE_TESTS", Some("0"))
        .define("NDEBUG", Some("0"))
        .compile("libreliable.a");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Build the wrapper bindings librt_transports
    let bindings = bindgen::Builder::default()
        .header("reliable.h")
        //.rustfmt_bindings(true)
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_path.join("private_bindings.rs"))
        .expect("Couldn't write bindings!");
}
