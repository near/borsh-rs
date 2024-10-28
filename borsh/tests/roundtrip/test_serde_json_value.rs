use serde_json::json;

#[test]
fn test_json_value() {
    let original = json!({
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

    let serialized = borsh::to_vec(&original).unwrap();

    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(serialized);

    let deserialized: serde_json::Value = borsh::from_slice(&serialized).unwrap();

    assert_eq!(original, deserialized);
}
