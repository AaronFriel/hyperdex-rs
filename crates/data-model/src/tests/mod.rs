use super::*;

mod hegel;

#[test]
fn record_from_attributes_uses_attribute_names() {
    let record = Record::from_attributes(
        Bytes::from_static(b"key"),
        vec![
            Attribute {
                name: "name".to_owned(),
                value: Value::String("ada".to_owned()),
            },
            Attribute {
                name: "age".to_owned(),
                value: Value::Int(37),
            },
        ],
    );

    assert_eq!(
        record.attributes.get("name"),
        Some(&Value::String("ada".to_owned()))
    );
    assert_eq!(record.attributes.get("age"), Some(&Value::Int(37)));
}

#[test]
fn parse_hyhac_default_space_description() {
    let schema = parse_hyperdex_space(
        "space profiles\n\
         key username\n\
         attributes\n\
            string first,\n\
            int profile_views,\n\
            map(string, int) upvotes\n\
         tolerate 0 failures\n",
    )
    .unwrap();

    assert_eq!(schema.name, "profiles");
    assert_eq!(schema.key_attribute, "username");
    assert_eq!(schema.options.fault_tolerance, 0);
    assert_eq!(schema.attributes.len(), 3);
    assert_eq!(schema.attributes[0].name, "first");
    assert_eq!(
        schema.attributes[2].kind,
        ValueKind::Map {
            key: Box::new(ValueKind::String),
            value: Box::new(ValueKind::Int),
        }
    );
}

#[test]
fn format_hyperdex_space_round_trips_nested_schema() {
    let schema = Space {
        name: "profiles".to_owned(),
        key_attribute: "username".to_owned(),
        attributes: vec![
            AttributeDefinition {
                name: "display_name".to_owned(),
                kind: ValueKind::String,
            },
            AttributeDefinition {
                name: "enabled".to_owned(),
                kind: ValueKind::Bool,
            },
            AttributeDefinition {
                name: "last_seen".to_owned(),
                kind: ValueKind::Timestamp(TimeUnit::Minute),
            },
            AttributeDefinition {
                name: "counts".to_owned(),
                kind: ValueKind::Map {
                    key: Box::new(ValueKind::String),
                    value: Box::new(ValueKind::Map {
                        key: Box::new(ValueKind::Int),
                        value: Box::new(ValueKind::Bool),
                    }),
                },
            },
        ],
        subspaces: vec![Subspace {
            dimensions: vec!["display_name".to_owned(), "last_seen".to_owned()],
        }],
        options: SpaceOptions {
            fault_tolerance: 2,
            partitions: 32,
            schema_format: SchemaFormat::HyperDexDsl,
        },
    };

    let rendered = format_hyperdex_space(&schema);

    assert_eq!(parse_hyperdex_space(&rendered).unwrap(), schema);
}
