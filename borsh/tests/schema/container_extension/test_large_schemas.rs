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

/// Nesting deep enough to overflow the stack, if each level took a frame.
const DEEP: usize = 100_000;

/// `C0 = ()` and `Ck` wraps `C(k-1)` in each kind of zero-sized definition in
/// turn, with a sequence of the deepest one: every walk follows the chain to
/// the bottom.
#[test]
fn deep_definitions() {
    let mut definitions = BTreeMap::new();
    definitions.insert("C0".to_string(), Definition::Primitive(0));
    for k in 1..=DEEP {
        let inner = format!("C{}", k - 1);
        let definition = match k % 5 {
            0 => Definition::Tuple {
                elements: vec![inner],
            },
            1 => Definition::Struct {
                fields: Fields::UnnamedFields(vec![inner]),
            },
            2 => Definition::Struct {
                fields: Fields::NamedFields(vec![("f".to_string(), inner)]),
            },
            3 => Definition::Enum {
                tag_width: 0,
                variants: vec![(0, "V".to_string(), inner)],
            },
            _ => Definition::Sequence {
                length_width: 0,
                length_range: 1..=1,
                elements: inner,
            },
        };
        definitions.insert(format!("C{}", k), definition);
    }
    let top = format!("C{}", DEEP);
    definitions.insert(
        "Vec".to_string(),
        Definition::Sequence {
            length_width: 4,
            length_range: 0..=u32::MAX as u64,
            elements: top.clone(),
        },
    );
    let elements = vec![top, "Vec".to_string()];
    definitions.insert("Root".to_string(), Definition::Tuple { elements });

    let schema = BorshSchemaContainer::new("Root".to_string(), definitions);
    assert_eq!(
        Err(SchemaContainerValidateError::ZSTSequence("Vec".to_string())),
        schema.validate()
    );
    assert_eq!(Ok(4), schema.max_serialized_size());
}

/// A cycle `Ck = struct(C(k+1))` with a sequence of each declaration on it:
/// every zero-size check runs into the cycle, which is walked once rather than
/// once per sequence.
#[test]
fn sequences_into_a_cycle() {
    let mut definitions = BTreeMap::new();
    let mut sequences = Vec::new();
    for k in 0..DEEP {
        let fields = Fields::UnnamedFields(vec![format!("C{}", (k + 1) % DEEP)]);
        definitions.insert(format!("C{}", k), Definition::Struct { fields });
        let sequence = format!("Vec<C{}>", k);
        definitions.insert(
            sequence.clone(),
            Definition::Sequence {
                length_width: 4,
                length_range: 0..=u32::MAX as u64,
                elements: format!("C{}", k),
            },
        );
        sequences.push(sequence);
    }
    definitions.insert("Root".to_string(), Definition::Tuple {
        elements: sequences,
    });

    let schema = BorshSchemaContainer::new("Root".to_string(), definitions);
    assert_eq!(Ok(()), schema.validate());
    assert_eq!(
        Err(SchemaMaxSerializedSizeError::Recursive),
        schema.max_serialized_size()
    );
}
