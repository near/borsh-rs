use std::collections::HashMap;

use borsh::BorshSerialize;

mod serde_json_value {
    pub use ser::serialize_value;
    mod ser {
        use borsh::{
            io::{ErrorKind, Result, Write},
            BorshSerialize,
        };
        use core::convert::TryFrom;

        /// this is mutually recursive with serialize_array and serialize_map
        pub fn serialize_value<W: Write>(value: &serde_json::Value, writer: &mut W) -> Result<()> {
            match value {
                serde_json::Value::Null => 0_u8.serialize(writer),
                serde_json::Value::Bool(b) => {
                    1_u8.serialize(writer)?;
                    b.serialize(writer)
                }
                serde_json::Value::Number(n) => {
                    2_u8.serialize(writer)?;
                    serialize_number(n, writer)
                }
                serde_json::Value::String(s) => {
                    3_u8.serialize(writer)?;
                    s.serialize(writer)
                }
                serde_json::Value::Array(a) => {
                    4_u8.serialize(writer)?;
                    serialize_array(a, writer)
                }
                serde_json::Value::Object(o) => {
                    5_u8.serialize(writer)?;
                    serialize_map(o, writer)
                }
            }
        }

        /// this is mutually recursive with serialize_value
        fn serialize_array<W: Write>(
            array: &Vec<serde_json::Value>,
            writer: &mut W,
        ) -> Result<()> {
            writer.write_all(
                &(u32::try_from(array.len()).map_err(|_| ErrorKind::InvalidData)?).to_le_bytes(),
            )?;
            for item in array {
                serialize_value(&item, writer)?;
            }
            Ok(())
        }

        /// this is mutually recursive with serialize_value
        fn serialize_map<W: Write>(
            map: &serde_json::Map<String, serde_json::Value>,
            writer: &mut W,
        ) -> Result<()> {
            // The implementation here is identical to that of BTreeMap<String, serde_json::Value>.
            u32::try_from(map.len())
                .map_err(|_| ErrorKind::InvalidData)?
                .serialize(writer)?;

            for (key, value) in map {
                key.serialize(writer)?;
                serialize_value(&value, writer)?;
            }

            Ok(())
        }

        fn serialize_number<W: Write>(number: &serde_json::Number, writer: &mut W) -> Result<()> {
            // A JSON number can either be a non-negative integer (represented in
            // serde_json by a u64), a negative integer (by an i64), or a non-integer
            // (by an f64).
            // We identify these cases with the following single-byte discriminants:
            // 0 - u64
            // 1 - i64
            // 2 - f64
            if let Some(u) = number.as_u64() {
                0_u8.serialize(writer)?;
                return u.serialize(writer);
            }

            if let Some(i) = number.as_i64() {
                1_u8.serialize(writer)?;
                return i.serialize(writer);
            }

            if let Some(f) = number.as_f64() {
                2_u8.serialize(writer)?;
                return f.serialize(writer);
            }

            unreachable!("number is neither a u64, i64, nor f64");
        }
    }
}

mod map_of_serde_json_value {
    pub use ser::serialize_map;

    mod ser {

        use borsh::{
            io::{ErrorKind, Result, Write},
            BorshSerialize,
        };
        use core::convert::TryFrom;
        use std::collections::HashMap;

        pub fn serialize_map<W: Write>(value: &HashMap<String, serde_json::Value>, writer: &mut W) -> Result<()> {
            let mut vec = value.iter().collect::<Vec<_>>();
            vec.sort_by(|(a, _), (b, _)| a.cmp(b));
            u32::try_from(vec.len())
                .map_err(|_| ErrorKind::InvalidData)?
                .serialize(writer)?;
            for kv in vec {
                kv.0.serialize(writer)?;
                crate::serde_json_value::serialize_value(kv.1, writer)?;
            }
            Ok(())
        }
        
    }
    
} 

#[derive(BorshSerialize)]
struct SerdeJsonAsField {
    #[borsh(
        serialize_with = "map_of_serde_json_value::serialize_map",
    )]
    examples: HashMap<String, serde_json::Value>,
}



fn main() {
    let original = serde_json::json!({
        "null": null,
        "true": true,
        "false": false,
        "zero": 0,
        "positive_integer": 12345,
        "negative_integer": -88888,
        "positive_float": 123.45,
        "negative_float": -888.88,
        "positive_max": 1.7976931348623157e+308,
        "negative_max": -1.7976931348623157e+308,
        "string": "Larry",
        "array_of_nulls": [null, null, null],
        "array_of_numbers": [0, -1, 1, 1.1, -1.1, 34798324],
        "array_of_strings": ["Larry", "Jake", "Pumpkin"],
        "array_of_arrays": [
            [1, 2, 3],
            [4, 5, 6],
            [7, 8, 9]
        ],
        "array_of_objects": [
            {
                "name": "Larry",
                "age": 30
            },
            {
                "name": "Jake",
                "age": 7
            },
            {
                "name": "Pumpkin",
                "age": 8
            }
        ],
        "object": {
            "name": "Larry",
            "age": 30,
            "pets": [
                {
                    "name": "Jake",
                    "age": 7
                },
                {
                    "name": "Pumpkin",
                    "age": 8
                }
            ]
        }
    });

    let mut examples = HashMap::new();
    examples.insert("Larry Jake Pumpkin".into(), original);

    let complex_struct = SerdeJsonAsField {
        examples,
    };
    let serialized = borsh::to_vec(&complex_struct).unwrap();

    println!("{:#?}", serialized);
}
