use cfg_aliases::cfg_aliases;

fn main() {
    cfg_aliases! {
        hash_collections: { any(feature = "hashbrown", feature = "std") },
        bounded: { feature = "unstable__bounded" },
    }
}
