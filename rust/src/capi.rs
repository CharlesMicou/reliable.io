#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(improper_ctypes)]
#![cfg_attr(feature="cargo-clippy", allow(clippy, clippy_correctness, clippy_style, clippy_pedantic, clippy_perf))]
include!(concat!(env!("OUT_DIR"), "/private_bindings.rs"));