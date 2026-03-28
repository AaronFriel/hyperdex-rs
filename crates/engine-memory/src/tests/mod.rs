use super::*;
use data_model::{Attribute, Check, Mutation, Predicate};

mod hegel;

#[test]
fn conditional_put_respects_checks() {
    let engine = MemoryEngine::new();
    engine.create_space("profiles".to_owned()).unwrap();
    engine
        .put(
            "profiles",
            Bytes::from_static(b"ada"),
            &[Mutation::Set(Attribute {
                name: "count".to_owned(),
                value: Value::Int(2),
            })],
        )
        .unwrap();

    let result = engine
        .conditional_put(
            "profiles",
            Bytes::from_static(b"ada"),
            &[Check {
                attribute: "count".to_owned(),
                predicate: Predicate::Equal,
                value: Value::Int(3),
            }],
            &[Mutation::Set(Attribute {
                name: "name".to_owned(),
                value: Value::String("Ada".to_owned()),
            })],
        )
        .unwrap();

    assert_eq!(result, WriteResult::ConditionFailed);
    assert!(
        !engine
            .get("profiles", b"ada")
            .unwrap()
            .unwrap()
            .attributes
            .contains_key("name")
    );
}

#[test]
fn failed_conditional_put_does_not_create_missing_record() {
    let engine = MemoryEngine::new();
    engine.create_space("profiles".to_owned()).unwrap();

    let result = engine
        .conditional_put(
            "profiles",
            Bytes::from_static(b"ada"),
            &[Check {
                attribute: "count".to_owned(),
                predicate: Predicate::Equal,
                value: Value::Int(3),
            }],
            &[Mutation::Set(Attribute {
                name: "name".to_owned(),
                value: Value::String("Ada".to_owned()),
            })],
        )
        .unwrap();

    assert_eq!(result, WriteResult::ConditionFailed);
    assert!(engine.get("profiles", b"ada").unwrap().is_none());
}
