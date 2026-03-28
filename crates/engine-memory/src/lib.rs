use std::collections::BTreeMap;

use anyhow::Result;
use bytes::Bytes;
use data_model::{Attribute, Check, Mutation, NumericOp, Predicate, Record, SpaceName, Value};
use parking_lot::RwLock;
use storage_core::{StorageEngine, StorageError, WriteResult};

#[derive(Default)]
pub struct MemoryEngine {
    spaces: RwLock<BTreeMap<SpaceName, BTreeMap<Vec<u8>, Record>>>,
}

impl MemoryEngine {
    pub fn new() -> Self {
        Self::default()
    }
}

impl StorageEngine for MemoryEngine {
    fn put(&self, space: &str, key: Bytes, mutations: &[Mutation]) -> Result<WriteResult> {
        let mut guard = self.spaces.write();
        let records = guard
            .get_mut(space)
            .ok_or_else(|| StorageError::UnknownSpace(space.to_owned()))?;
        let record = records
            .entry(key.to_vec())
            .or_insert_with(|| Record::new(key.clone()));
        apply_mutations(record, mutations)?;
        Ok(WriteResult::Written)
    }

    fn get(&self, space: &str, key: &[u8]) -> Result<Option<Record>> {
        let guard = self.spaces.read();
        Ok(guard
            .get(space)
            .and_then(|records| records.get(key).cloned()))
    }

    fn delete(&self, space: &str, key: &[u8]) -> Result<WriteResult> {
        let mut guard = self.spaces.write();
        let Some(records) = guard.get_mut(space) else {
            return Err(StorageError::UnknownSpace(space.to_owned()).into());
        };
        Ok(if records.remove(key).is_some() {
            WriteResult::Written
        } else {
            WriteResult::Missing
        })
    }

    fn conditional_put(
        &self,
        space: &str,
        key: Bytes,
        checks: &[Check],
        mutations: &[Mutation],
    ) -> Result<WriteResult> {
        let mut guard = self.spaces.write();
        let records = guard
            .get_mut(space)
            .ok_or_else(|| StorageError::UnknownSpace(space.to_owned()))?;
        let key_vec = key.to_vec();

        match records.get(&key_vec) {
            Some(record) if !record_matches(record, checks) => {
                return Ok(WriteResult::ConditionFailed);
            }
            None if !checks.is_empty() => return Ok(WriteResult::ConditionFailed),
            _ => {}
        }

        let record = records
            .entry(key_vec)
            .or_insert_with(|| Record::new(key.clone()));

        apply_mutations(record, mutations)?;
        Ok(WriteResult::Written)
    }

    fn search(&self, space: &str, checks: &[Check]) -> Result<Vec<Record>> {
        let guard = self.spaces.read();
        let records = guard
            .get(space)
            .ok_or_else(|| StorageError::UnknownSpace(space.to_owned()))?;
        Ok(records
            .values()
            .filter(|record| record_matches(record, checks))
            .cloned()
            .collect())
    }

    fn count(&self, space: &str, checks: &[Check]) -> Result<u64> {
        Ok(self.search(space, checks)?.len() as u64)
    }

    fn delete_matching(&self, space: &str, checks: &[Check]) -> Result<u64> {
        let mut guard = self.spaces.write();
        let records = guard
            .get_mut(space)
            .ok_or_else(|| StorageError::UnknownSpace(space.to_owned()))?;
        let doomed: Vec<Vec<u8>> = records
            .iter()
            .filter(|&(_, record)| record_matches(record, checks))
            .map(|(key, _)| key.clone())
            .collect();
        let count = doomed.len() as u64;

        for key in doomed {
            records.remove(&key);
        }

        Ok(count)
    }

    fn spaces(&self) -> Result<Vec<SpaceName>> {
        Ok(self.spaces.read().keys().cloned().collect())
    }

    fn create_space(&self, space: SpaceName) -> Result<()> {
        self.spaces.write().entry(space).or_default();
        Ok(())
    }

    fn drop_space(&self, space: &str) -> Result<()> {
        self.spaces.write().remove(space);
        Ok(())
    }
}

fn apply_mutations(record: &mut Record, mutations: &[Mutation]) -> Result<()> {
    for mutation in mutations {
        match mutation {
            Mutation::Set(Attribute { name, value }) => {
                record.attributes.insert(name.clone(), value.clone());
            }
            Mutation::Numeric {
                attribute,
                op,
                operand,
            } => {
                let current = record
                    .attributes
                    .entry(attribute.clone())
                    .or_insert(Value::Int(0));
                let Value::Int(current_value) = current else {
                    return Err(StorageError::NonNumericValue.into());
                };
                *current_value = apply_numeric(*current_value, *op, *operand);
            }
            Mutation::MapSet {
                attribute,
                map_key,
                value,
            } => {
                let current = record
                    .attributes
                    .entry(attribute.clone())
                    .or_insert_with(|| Value::Map(BTreeMap::new()));
                let Value::Map(map) = current else {
                    return Err(StorageError::NonNumericValue.into());
                };
                map.insert(map_key.clone(), value.clone());
            }
            Mutation::MapNumeric {
                attribute,
                map_key,
                op,
                operand,
            } => {
                let current = record
                    .attributes
                    .entry(attribute.clone())
                    .or_insert_with(|| Value::Map(BTreeMap::new()));
                let Value::Map(map) = current else {
                    return Err(StorageError::NonNumericValue.into());
                };
                let map_value = map.entry(map_key.clone()).or_insert(Value::Int(0));
                let Value::Int(current_value) = map_value else {
                    return Err(StorageError::NonNumericValue.into());
                };
                *current_value = apply_numeric(*current_value, *op, *operand);
            }
        }
    }

    Ok(())
}

fn apply_numeric(current: i64, op: NumericOp, operand: i64) -> i64 {
    match op {
        NumericOp::Add => current.saturating_add(operand),
        NumericOp::Sub => current.saturating_sub(operand),
        NumericOp::Mul => current.saturating_mul(operand),
        NumericOp::Div => current / operand,
        NumericOp::Mod => current % operand,
        NumericOp::And => current & operand,
        NumericOp::Or => current | operand,
        NumericOp::Xor => current ^ operand,
    }
}

fn record_matches(record: &Record, checks: &[Check]) -> bool {
    checks.iter().all(|check| {
        let Some(value) = record.attributes.get(&check.attribute) else {
            return false;
        };
        matches_predicate(value, check.predicate, &check.value)
    })
}

fn matches_predicate(left: &Value, predicate: Predicate, right: &Value) -> bool {
    match predicate {
        Predicate::Equal => left == right,
        Predicate::LessThan => left < right,
        Predicate::LessThanOrEqual => left <= right,
        Predicate::GreaterThan => left > right,
        Predicate::GreaterThanOrEqual => left >= right,
    }
}

#[cfg(test)]
mod tests;
