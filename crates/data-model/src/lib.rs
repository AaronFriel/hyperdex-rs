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
    pub options: SpaceOptions,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributeDefinition {
    pub name: AttributeName,
    pub kind: ValueKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueKind {
    Bool,
    Int,
    Float,
    Bytes,
    String,
    Document,
    Timestamp(TimeUnit),
    List(Box<ValueKind>),
    Set(Box<ValueKind>),
    Map {
        key: Box<ValueKind>,
        value: Box<ValueKind>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeUnit {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpaceOptions {
    pub fault_tolerance: u32,
    pub partitions: u32,
    pub schema_format: SchemaFormat,
}

#[derive(Debug, Error)]
pub enum DataModelError {
    #[error("attribute {0} is missing")]
    MissingAttribute(String),
    #[error("value type mismatch")]
    TypeMismatch,
    #[error("invalid schema: {0}")]
    InvalidSchema(String),
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

impl Default for SpaceOptions {
    fn default() -> Self {
        Self {
            fault_tolerance: 0,
            partitions: 64,
            schema_format: SchemaFormat::HyperDexDsl,
        }
    }
}

pub fn parse_hyperdex_space(input: &str) -> Result<Space, DataModelError> {
    let mut name = None;
    let mut key_attribute = None;
    let mut attributes = Vec::new();
    let mut subspaces = Vec::new();
    let mut options = SpaceOptions::default();
    let mut in_attributes = false;

    for raw_line in input.lines() {
        let line = raw_line.trim();

        if line.is_empty() {
            continue;
        }

        if let Some(rest) = line.strip_prefix("space ") {
            name = Some(rest.trim().to_owned());
            in_attributes = false;
            continue;
        }

        if let Some(rest) = line.strip_prefix("key ") {
            key_attribute = Some(rest.trim().to_owned());
            in_attributes = false;
            continue;
        }

        if line == "attributes" {
            in_attributes = true;
            continue;
        }

        if let Some(rest) = line.strip_prefix("subspace ") {
            in_attributes = false;
            let dimensions = rest
                .split(',')
                .map(|part| part.trim().to_owned())
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>();
            subspaces.push(Subspace { dimensions });
            continue;
        }

        if let Some(rest) = line.strip_prefix("tolerate ") {
            in_attributes = false;
            let count = rest
                .split_whitespace()
                .next()
                .ok_or_else(|| DataModelError::InvalidSchema(line.to_owned()))?;
            options.fault_tolerance = count
                .parse()
                .map_err(|_| DataModelError::InvalidSchema(line.to_owned()))?;
            continue;
        }

        if let Some(rest) = line.strip_prefix("create ") {
            in_attributes = false;
            let count = rest
                .split_whitespace()
                .next()
                .ok_or_else(|| DataModelError::InvalidSchema(line.to_owned()))?;
            options.partitions = count
                .parse()
                .map_err(|_| DataModelError::InvalidSchema(line.to_owned()))?;
            continue;
        }

        if in_attributes {
            let trimmed = line.trim_end_matches(',');
            attributes.push(parse_attribute_definition(trimmed)?);
        }
    }

    let name =
        name.ok_or_else(|| DataModelError::InvalidSchema("missing space name".to_owned()))?;
    let key_attribute = key_attribute
        .ok_or_else(|| DataModelError::InvalidSchema("missing key attribute".to_owned()))?;

    if attributes.is_empty() {
        return Err(DataModelError::InvalidSchema(
            "space must declare at least one attribute".to_owned(),
        ));
    }

    Ok(Space {
        name,
        key_attribute,
        attributes,
        subspaces,
        options,
    })
}

fn parse_attribute_definition(input: &str) -> Result<AttributeDefinition, DataModelError> {
    let mut depth = 0usize;
    let mut split_at = None;

    for (idx, ch) in input.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ' ' if depth == 0 => {
                split_at = Some(idx);
                break;
            }
            _ => {}
        }
    }

    let Some(split_at) = split_at else {
        return Ok(AttributeDefinition {
            name: input.to_owned(),
            kind: ValueKind::String,
        });
    };

    let kind = parse_value_kind(input[..split_at].trim())?;
    let name = input[split_at..].trim().to_owned();

    Ok(AttributeDefinition { name, kind })
}

fn parse_value_kind(input: &str) -> Result<ValueKind, DataModelError> {
    match input {
        "string" => return Ok(ValueKind::String),
        "int" | "int64" => return Ok(ValueKind::Int),
        "float" => return Ok(ValueKind::Float),
        "document" => return Ok(ValueKind::Document),
        "bytes" => return Ok(ValueKind::Bytes),
        _ => {}
    }

    if let Some(inner) = input
        .strip_prefix("list(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return Ok(ValueKind::List(Box::new(parse_value_kind(inner.trim())?)));
    }

    if let Some(inner) = input
        .strip_prefix("set(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return Ok(ValueKind::Set(Box::new(parse_value_kind(inner.trim())?)));
    }

    if let Some(inner) = input
        .strip_prefix("map(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let mut parts = inner.splitn(2, ',');
        let key = parts
            .next()
            .ok_or_else(|| DataModelError::InvalidSchema(input.to_owned()))?;
        let value = parts
            .next()
            .ok_or_else(|| DataModelError::InvalidSchema(input.to_owned()))?;
        return Ok(ValueKind::Map {
            key: Box::new(parse_value_kind(key.trim())?),
            value: Box::new(parse_value_kind(value.trim())?),
        });
    }

    if let Some(inner) = input
        .strip_prefix("timestamp(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let unit = match inner.trim() {
            "second" => TimeUnit::Second,
            "minute" => TimeUnit::Minute,
            "hour" => TimeUnit::Hour,
            "day" => TimeUnit::Day,
            "week" => TimeUnit::Week,
            "month" => TimeUnit::Month,
            _ => {
                return Err(DataModelError::InvalidSchema(format!(
                    "unknown timestamp unit: {inner}"
                )));
            }
        };
        return Ok(ValueKind::Timestamp(unit));
    }

    Err(DataModelError::InvalidSchema(format!(
        "unsupported type expression: {input}"
    )))
}

#[cfg(test)]
mod tests;
