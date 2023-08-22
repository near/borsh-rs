//! Generate `BorshSchemaCointainer` for `BorshSchemaContainer` and save it into a file.

#![cfg_attr(not(feature = "std"), no_std)]
use borsh::{schema_container_of, BorshSerialize};
use std::fs::File;
use std::io::Write;

fn main() {
    let container = schema_container_of::<borsh::schema::BorshSchemaContainer>();

    println!("{:#?}", container);
    let data = container
        .try_to_vec()
        .expect("Failed to serialize BorshSchemaContainer");
    let mut file = File::create("schema_schema.dat").expect("Failed to create file");
    file.write_all(&data).expect("Failed to write file");
}
