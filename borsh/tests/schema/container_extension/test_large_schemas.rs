use crate::common_macro::schema_imports::*;

/// `D0 = ()` and `Dk = (D(k-1), D(k-1))`: following every reference takes
/// `2^64` steps.
#[test]
fn shared_definitions() {
    let mut definitions = BTreeMap::new();
    definitions.insert("D0".to_string(), Definition::Primitive(0));
    for k in 1..=64 {
        let below = format!("D{}", k - 1);
        let elements = vec![below.clone(), below];
        definitions.insert(format!("D{}", k), Definition::Tuple { elements });
    }
    definitions.insert(
        "Vec<D64>".to_string(),
        Definition::Sequence {
            length_width: 4,
            length_range: 0..=u32::MAX as u64,
            elements: "D64".to_string(),
        },
    );
    let elements = vec!["D64".to_string(), "Vec<D64>".to_string()];
    definitions.insert("Root".to_string(), Definition::Tuple { elements });

    let schema = BorshSchemaContainer::new("Root".to_string(), definitions);
    assert_eq!(
        Err(SchemaContainerValidateError::ZSTSequence(
            "Vec<D64>".to_string()
        )),
        schema.validate()
    );
    assert_eq!(Ok(4), schema.max_serialized_size());
}
