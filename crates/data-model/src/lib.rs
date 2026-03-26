use std::collections::{BTreeMap, BTreeSet};

use bytes::Bytes;
use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type SpaceName = String;
pub type AttributeName = String;
pub type NodeId = u64;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(OrderedFloat<f64>),
    Bytes(Bytes),
    String(String),
    List(Vec<Value>),
    Set(BTreeSet<Value>),
    Map(BTreeMap<Value, Value>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attribute {
    pub name: AttributeName,
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub key: Bytes,
    pub attributes: BTreeMap<AttributeName, Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Space {
    pub name: SpaceName,
    pub key_attribute: AttributeName,
    pub attributes: Vec<AttributeDefinition>,
    pub subspaces: Vec<Subspace>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributeDefinition {
    pub name: AttributeName,
    pub kind: ValueKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueKind {
    Bool,
    Int,
    Float,
    Bytes,
    String,
    List,
    Set,
    Map,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subspace {
    pub dimensions: Vec<AttributeName>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Predicate {
    Equal,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    pub attribute: AttributeName,
    pub predicate: Predicate,
    pub value: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NumericOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    Xor,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mutation {
    Set(Attribute),
    Numeric {
        attribute: AttributeName,
        op: NumericOp,
        operand: i64,
    },
    MapSet {
        attribute: AttributeName,
        map_key: Value,
        value: Value,
    },
    MapNumeric {
        attribute: AttributeName,
        map_key: Value,
        op: NumericOp,
        operand: i64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchemaFormat {
    HyperDexDsl,
    Protobuf,
    Json,
}

#[derive(Debug, Error)]
pub enum DataModelError {
    #[error("attribute {0} is missing")]
    MissingAttribute(String),
    #[error("value type mismatch")]
    TypeMismatch,
}

impl Record {
    pub fn new(key: Bytes) -> Self {
        Self {
            key,
            attributes: BTreeMap::new(),
        }
    }

    pub fn from_attributes(key: Bytes, attributes: Vec<Attribute>) -> Self {
        let mut record = Self::new(key);

        for attribute in attributes {
            record.attributes.insert(attribute.name, attribute.value);
        }

        record
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        assert_eq!(record.attributes.get("name"), Some(&Value::String("ada".to_owned())));
        assert_eq!(record.attributes.get("age"), Some(&Value::Int(37)));
    }
}
