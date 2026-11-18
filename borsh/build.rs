//! `hash_collections`: the HashMap/HashSet impls exist (with `std` or `hashbrown`). The tests
//! use this cfg too.
fn main() {
    println!("cargo:rustc-check-cfg=cfg(hash_collections)");
    if std::env::var_os("CARGO_FEATURE_STD").is_some()
        || std::env::var_os("CARGO_FEATURE_HASHBROWN").is_some()
    {
        println!("cargo:rustc-cfg=hash_collections");
    }
}
