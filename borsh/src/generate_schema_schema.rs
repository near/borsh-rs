//! Writes the schema of `BorshSchemaContainer` to `schema_schema.dat`.
use std::io::Write;

fn main() {
    let container = borsh::schema_container_of::<borsh::schema::BorshSchemaContainer>();
    println!("{:#?}", container);
    let data = borsh::to_vec(&container).expect("Failed to serialize BorshSchemaContainer");
    let mut file = std::fs::File::create("schema_schema.dat").expect("Failed to create file");
    file.write_all(&data).expect("Failed to write file");
}
