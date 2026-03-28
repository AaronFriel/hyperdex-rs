#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use ::hegel::{TestCase, generators as gs};
use anyhow::Result;
use bytes::Bytes;
use cluster_config::ClusterNode;
use control_plane::Catalog;
use data_model::{
    Attribute, AttributeDefinition, Check, Mutation, NumericOp, Predicate, Record, SchemaFormat,
    Space, SpaceOptions, Subspace, Value, ValueKind,
};
use placement_core::{
    ClusterLayout, PlacementDecision, PlacementError, PlacementStrategy, Result as PlacementResult,
};
use storage_core::{StorageEngine, WriteResult};

use crate::DataPlane;

const ROUTE_OK_MARKER: u8 = 0x01;
const ROUTE_FAIL_MARKER: u8 = 0xff;

#[derive(Clone, Debug)]
enum GeneratedOperation {
    Put {
        space: String,
        key: Bytes,
        mutations: Vec<Mutation>,
    },
    Get {
        space: String,
        key: Vec<u8>,
    },
    Delete {
        space: String,
        key: Vec<u8>,
    },
    ConditionalPut {
        space: String,
        key: Bytes,
        checks: Vec<Check>,
        mutations: Vec<Mutation>,
    },
    Search {
        space: String,
        checks: Vec<Check>,
    },
    Count {
        space: String,
        checks: Vec<Check>,
    },
    DeleteMatching {
        space: String,
        checks: Vec<Check>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    CatalogGetSpace(String),
    CatalogLayout,
    PlacementLocate {
        key: Vec<u8>,
        layout: ClusterLayout,
    },
    StoragePut {
        space: String,
        key: Vec<u8>,
        mutations: Vec<Mutation>,
    },
    StorageGet {
        space: String,
        key: Vec<u8>,
    },
    StorageDelete {
        space: String,
        key: Vec<u8>,
    },
    StorageConditionalPut {
        space: String,
        key: Vec<u8>,
        checks: Vec<Check>,
        mutations: Vec<Mutation>,
    },
    StorageSearch {
        space: String,
        checks: Vec<Check>,
    },
    StorageCount {
        space: String,
        checks: Vec<Check>,
    },
    StorageDeleteMatching {
        space: String,
        checks: Vec<Check>,
    },
}

struct SpyCatalog {
    events: Arc<Mutex<Vec<Event>>>,
    layout: ClusterLayout,
    spaces: BTreeMap<String, Space>,
}

struct SpyPlacement {
    events: Arc<Mutex<Vec<Event>>>,
}

struct SpyStorage {
    events: Arc<Mutex<Vec<Event>>>,
}

impl SpyCatalog {
    fn new(events: Arc<Mutex<Vec<Event>>>, layout: ClusterLayout, spaces: Vec<&str>) -> Self {
        Self {
            events,
            layout,
            spaces: spaces
                .into_iter()
                .map(|space| (space.to_owned(), test_space(space)))
                .collect(),
        }
    }

    fn push_event(&self, event: Event) {
        self.events.lock().unwrap().push(event);
    }
}

impl Catalog for SpyCatalog {
    fn create_space(&self, _space: Space) -> Result<()> {
        Ok(())
    }

    fn drop_space(&self, _name: &str) -> Result<()> {
        Ok(())
    }

    fn list_spaces(&self) -> Result<Vec<String>> {
        Ok(self.spaces.keys().cloned().collect())
    }

    fn get_space(&self, name: &str) -> Result<Option<Space>> {
        self.push_event(Event::CatalogGetSpace(name.to_owned()));
        Ok(self.spaces.get(name).cloned())
    }

    fn register_daemon(&self, _node: ClusterNode) -> Result<bool> {
        Ok(false)
    }

    fn replace_daemons(&self, _nodes: Vec<ClusterNode>) -> Result<bool> {
        Ok(false)
    }

    fn layout(&self) -> Result<ClusterLayout> {
        self.push_event(Event::CatalogLayout);
        Ok(self.layout.clone())
    }
}

impl PlacementStrategy for SpyPlacement {
    fn locate(&self, key: &[u8], layout: &ClusterLayout) -> PlacementResult<PlacementDecision> {
        self.events.lock().unwrap().push(Event::PlacementLocate {
            key: key.to_vec(),
            layout: layout.clone(),
        });

        if route_fails(key) {
            return Err(PlacementError::EmptyLayout);
        }

        Ok(PlacementDecision {
            partition: usize::from(key.first().copied().unwrap_or_default()) % 7,
            partitions: 7,
            primary: layout.nodes[0],
            replicas: layout
                .nodes
                .iter()
                .copied()
                .take(layout.replicas.max(1))
                .collect(),
        })
    }

    fn name(&self) -> &'static str {
        "spy-placement"
    }
}

impl StorageEngine for SpyStorage {
    fn put(&self, space: &str, key: Bytes, mutations: &[Mutation]) -> Result<WriteResult> {
        self.events.lock().unwrap().push(Event::StoragePut {
            space: space.to_owned(),
            key: key.to_vec(),
            mutations: mutations.to_vec(),
        });
        Ok(expected_put_result(space, &key, mutations))
    }

    fn get(&self, space: &str, key: &[u8]) -> Result<Option<Record>> {
        self.events.lock().unwrap().push(Event::StorageGet {
            space: space.to_owned(),
            key: key.to_vec(),
        });
        Ok(expected_get_result(space, key))
    }

    fn delete(&self, space: &str, key: &[u8]) -> Result<WriteResult> {
        self.events.lock().unwrap().push(Event::StorageDelete {
            space: space.to_owned(),
            key: key.to_vec(),
        });
        Ok(expected_delete_result(space, key))
    }

    fn conditional_put(
        &self,
        space: &str,
        key: Bytes,
        checks: &[Check],
        mutations: &[Mutation],
    ) -> Result<WriteResult> {
        self.events
            .lock()
            .unwrap()
            .push(Event::StorageConditionalPut {
                space: space.to_owned(),
                key: key.to_vec(),
                checks: checks.to_vec(),
                mutations: mutations.to_vec(),
            });
        Ok(expected_conditional_put_result(
            space, &key, checks, mutations,
        ))
    }

    fn search(&self, space: &str, checks: &[Check]) -> Result<Vec<Record>> {
        self.events.lock().unwrap().push(Event::StorageSearch {
            space: space.to_owned(),
            checks: checks.to_vec(),
        });
        Ok(expected_search_result(space, checks))
    }

    fn count(&self, space: &str, checks: &[Check]) -> Result<u64> {
        self.events.lock().unwrap().push(Event::StorageCount {
            space: space.to_owned(),
            checks: checks.to_vec(),
        });
        Ok(expected_count_result(space, checks))
    }

    fn delete_matching(&self, space: &str, checks: &[Check]) -> Result<u64> {
        self.events
            .lock()
            .unwrap()
            .push(Event::StorageDeleteMatching {
                space: space.to_owned(),
                checks: checks.to_vec(),
            });
        Ok(expected_delete_matching_result(space, checks))
    }

    fn spaces(&self) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn create_space(&self, _space: String) -> Result<()> {
        Ok(())
    }

    fn drop_space(&self, _space: &str) -> Result<()> {
        Ok(())
    }
}

#[::hegel::composite]
fn generated_value(tc: TestCase) -> Value {
    let kind: u8 = tc.draw(gs::integers::<u8>().max_value(7));
    let seed_a: u16 = tc.draw(gs::integers::<u16>().max_value(255));
    let seed_b: u16 = tc.draw(gs::integers::<u16>().max_value(255));

    match kind {
        0 => Value::Null,
        1 => Value::Bool(seed_a % 2 == 0),
        2 => Value::Int(i64::from(seed_a) - 96),
        3 => Value::Bytes(Bytes::from(vec![seed_a as u8, seed_b as u8])),
        4 => Value::String(format!("v-{seed_a}-{seed_b}")),
        5 => Value::List(vec![
            Value::Int(i64::from(seed_a % 17)),
            Value::String(format!("item-{seed_b}")),
        ]),
        6 => Value::Set(BTreeSet::from([
            Value::Int(i64::from(seed_a % 13)),
            Value::String(format!("set-{seed_b}")),
        ])),
        _ => Value::Map(BTreeMap::from([
            (
                Value::String(format!("key-{seed_a}")),
                Value::Int(i64::from(seed_b % 19)),
            ),
            (
                Value::Int(i64::from(seed_b % 11)),
                Value::String(format!("value-{seed_a}")),
            ),
        ])),
    }
}

#[::hegel::composite]
fn generated_scalar_value(tc: TestCase) -> Value {
    match tc.draw(gs::integers::<u8>().max_value(2)) {
        0 => Value::Int(i64::from(tc.draw(gs::integers::<u8>())) - 64),
        1 => Value::String(format!(
            "k-{}",
            tc.draw::<u16>(gs::integers::<u16>().max_value(255))
        )),
        _ => Value::Bytes(Bytes::from(vec![
            tc.draw(gs::integers::<u8>()),
            tc.draw(gs::integers::<u8>()),
        ])),
    }
}

#[::hegel::composite]
fn generated_check(tc: TestCase) -> Check {
    let predicate = match tc.draw(gs::integers::<u8>().max_value(4)) {
        0 => Predicate::Equal,
        1 => Predicate::LessThan,
        2 => Predicate::LessThanOrEqual,
        3 => Predicate::GreaterThan,
        _ => Predicate::GreaterThanOrEqual,
    };

    Check {
        attribute: format!("attr_{}", tc.draw::<u8>(gs::integers::<u8>().max_value(5))),
        predicate,
        value: tc.draw(generated_value()),
    }
}

#[::hegel::composite]
fn generated_mutation(tc: TestCase) -> Mutation {
    let attribute = format!("attr_{}", tc.draw::<u8>(gs::integers::<u8>().max_value(5)));
    let op = match tc.draw(gs::integers::<u8>().max_value(7)) {
        0 => NumericOp::Add,
        1 => NumericOp::Sub,
        2 => NumericOp::Mul,
        3 => NumericOp::Div,
        4 => NumericOp::Mod,
        5 => NumericOp::And,
        6 => NumericOp::Or,
        _ => NumericOp::Xor,
    };
    let operand = i64::from(tc.draw::<u8>(gs::integers::<u8>().max_value(63))) - 31;

    match tc.draw(gs::integers::<u8>().max_value(3)) {
        0 => Mutation::Set(Attribute {
            name: attribute,
            value: tc.draw(generated_value()),
        }),
        1 => Mutation::Numeric {
            attribute,
            op,
            operand,
        },
        2 => Mutation::MapSet {
            attribute,
            map_key: tc.draw(generated_scalar_value()),
            value: tc.draw(generated_value()),
        },
        _ => Mutation::MapNumeric {
            attribute,
            map_key: tc.draw(generated_scalar_value()),
            op,
            operand,
        },
    }
}

#[::hegel::composite]
fn generated_operation(tc: TestCase) -> GeneratedOperation {
    let space = generated_space_name(
        tc.draw(gs::booleans()),
        tc.draw(gs::integers::<u8>().max_value(1)),
        tc.draw(gs::integers::<u8>().max_value(7)),
    );
    let route_should_fail = tc.draw(gs::booleans());
    let key_seed: u16 = tc.draw(gs::integers::<u16>().max_value(255));
    let key_tail: u16 = tc.draw(gs::integers::<u16>().max_value(255));
    let checks: Vec<Check> = tc.draw(gs::vecs(generated_check()).max_size(3));
    let mutations: Vec<Mutation> = tc.draw(gs::vecs(generated_mutation()).max_size(3));
    let key = generated_key(route_should_fail, key_seed, key_tail);
    let key_vec = key.to_vec();

    match tc.draw(gs::integers::<u8>().max_value(6)) {
        0 => GeneratedOperation::Put {
            space,
            key,
            mutations,
        },
        1 => GeneratedOperation::Get {
            space,
            key: key_vec,
        },
        2 => GeneratedOperation::Delete {
            space,
            key: key_vec,
        },
        3 => GeneratedOperation::ConditionalPut {
            space,
            key,
            checks,
            mutations,
        },
        4 => GeneratedOperation::Search { space, checks },
        5 => GeneratedOperation::Count { space, checks },
        _ => GeneratedOperation::DeleteMatching { space, checks },
    }
}

#[::hegel::test(test_cases = 60)]
fn hegel_data_plane_rejects_missing_spaces_and_only_routes_point_operations(tc: TestCase) {
    let operations: Vec<GeneratedOperation> =
        tc.draw(gs::vecs(generated_operation()).min_size(1).max_size(36));
    let events = Arc::new(Mutex::new(Vec::new()));
    let layout = ClusterLayout {
        replicas: 2,
        nodes: vec![11, 17, 23],
    };

    let plane = DataPlane::new(
        Arc::new(SpyCatalog::new(
            Arc::clone(&events),
            layout.clone(),
            vec!["profiles", "metrics"],
        )),
        Arc::new(SpyStorage {
            events: Arc::clone(&events),
        }),
        Arc::new(SpyPlacement {
            events: Arc::clone(&events),
        }),
    );

    for operation in operations {
        let start = events.lock().unwrap().len();

        match operation {
            GeneratedOperation::Put {
                space,
                key,
                mutations,
            } => {
                let result = plane.put(&space, key.clone(), &mutations);
                let key_vec = key.to_vec();
                assert_point_write(
                    &events,
                    start,
                    &space,
                    &key_vec,
                    &layout,
                    result,
                    Event::StoragePut {
                        space: space.clone(),
                        key: key_vec.clone(),
                        mutations: mutations.clone(),
                    },
                    expected_put_result(&space, &key, &mutations),
                );
            }
            GeneratedOperation::Get { space, key } => {
                let result = plane.get(&space, &key);
                assert_point_read(
                    &events,
                    start,
                    &space,
                    &key,
                    &layout,
                    result,
                    Event::StorageGet {
                        space: space.clone(),
                        key: key.clone(),
                    },
                    expected_get_result(&space, &key),
                );
            }
            GeneratedOperation::Delete { space, key } => {
                let result = plane.delete(&space, &key);
                assert_point_write(
                    &events,
                    start,
                    &space,
                    &key,
                    &layout,
                    result,
                    Event::StorageDelete {
                        space: space.clone(),
                        key: key.clone(),
                    },
                    expected_delete_result(&space, &key),
                );
            }
            GeneratedOperation::ConditionalPut {
                space,
                key,
                checks,
                mutations,
            } => {
                let result = plane.conditional_put(&space, key.clone(), &checks, &mutations);
                let key_vec = key.to_vec();
                assert_point_write(
                    &events,
                    start,
                    &space,
                    &key_vec,
                    &layout,
                    result,
                    Event::StorageConditionalPut {
                        space: space.clone(),
                        key: key_vec.clone(),
                        checks: checks.clone(),
                        mutations: mutations.clone(),
                    },
                    expected_conditional_put_result(&space, &key, &checks, &mutations),
                );
            }
            GeneratedOperation::Search { space, checks } => {
                let result = plane.search(&space, &checks);
                assert_non_point_operation(
                    &events,
                    start,
                    &space,
                    result,
                    Event::StorageSearch {
                        space: space.clone(),
                        checks: checks.clone(),
                    },
                    expected_search_result(&space, &checks),
                );
            }
            GeneratedOperation::Count { space, checks } => {
                let result = plane.count(&space, &checks);
                assert_non_point_operation(
                    &events,
                    start,
                    &space,
                    result,
                    Event::StorageCount {
                        space: space.clone(),
                        checks: checks.clone(),
                    },
                    expected_count_result(&space, &checks),
                );
            }
            GeneratedOperation::DeleteMatching { space, checks } => {
                let result = plane.delete_matching(&space, &checks);
                assert_non_point_operation(
                    &events,
                    start,
                    &space,
                    result,
                    Event::StorageDeleteMatching {
                        space: space.clone(),
                        checks: checks.clone(),
                    },
                    expected_delete_matching_result(&space, &checks),
                );
            }
        }
    }
}

fn assert_point_write(
    events: &Arc<Mutex<Vec<Event>>>,
    start: usize,
    space: &str,
    key: &[u8],
    layout: &ClusterLayout,
    result: Result<WriteResult>,
    storage_event: Event,
    expected_value: WriteResult,
) {
    let expected_events = expected_point_events(space, key, layout, storage_event);
    let actual_events = events_since(events, start);

    if !space_exists(space) {
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("space {space} does not exist")
        );
        assert_eq!(
            actual_events,
            vec![Event::CatalogGetSpace(space.to_owned())]
        );
        return;
    }

    if route_fails(key) {
        assert_eq!(
            result.unwrap_err().to_string(),
            PlacementError::EmptyLayout.to_string()
        );
        assert_eq!(
            actual_events,
            expected_events[..3].to_vec(),
            "point operations should stop before storage when placement fails"
        );
        return;
    }

    assert_eq!(result.unwrap(), expected_value);
    assert_eq!(actual_events, expected_events);
}

fn assert_point_read(
    events: &Arc<Mutex<Vec<Event>>>,
    start: usize,
    space: &str,
    key: &[u8],
    layout: &ClusterLayout,
    result: Result<Option<Record>>,
    storage_event: Event,
    expected_value: Option<Record>,
) {
    let expected_events = expected_point_events(space, key, layout, storage_event);
    let actual_events = events_since(events, start);

    if !space_exists(space) {
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("space {space} does not exist")
        );
        assert_eq!(
            actual_events,
            vec![Event::CatalogGetSpace(space.to_owned())]
        );
        return;
    }

    if route_fails(key) {
        assert_eq!(
            result.unwrap_err().to_string(),
            PlacementError::EmptyLayout.to_string()
        );
        assert_eq!(actual_events, expected_events[..3].to_vec());
        return;
    }

    assert_eq!(result.unwrap(), expected_value);
    assert_eq!(actual_events, expected_events);
}

fn assert_non_point_operation<T>(
    events: &Arc<Mutex<Vec<Event>>>,
    start: usize,
    space: &str,
    result: Result<T>,
    storage_event: Event,
    expected_value: T,
) where
    T: std::fmt::Debug + PartialEq,
{
    let actual_events = events_since(events, start);

    if !space_exists(space) {
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("space {space} does not exist")
        );
        assert_eq!(
            actual_events,
            vec![Event::CatalogGetSpace(space.to_owned())]
        );
        return;
    }

    assert_eq!(result.unwrap(), expected_value);
    assert_eq!(
        actual_events,
        vec![Event::CatalogGetSpace(space.to_owned()), storage_event]
    );
}

fn expected_point_events(
    space: &str,
    key: &[u8],
    layout: &ClusterLayout,
    storage_event: Event,
) -> Vec<Event> {
    vec![
        Event::CatalogGetSpace(space.to_owned()),
        Event::CatalogLayout,
        Event::PlacementLocate {
            key: key.to_vec(),
            layout: layout.clone(),
        },
        storage_event,
    ]
}

fn events_since(events: &Arc<Mutex<Vec<Event>>>, start: usize) -> Vec<Event> {
    events.lock().unwrap()[start..].to_vec()
}

fn space_exists(space: &str) -> bool {
    matches!(space, "profiles" | "metrics")
}

fn generated_space_name(existing: bool, existing_id: u8, missing_seed: u8) -> String {
    if existing {
        if existing_id % 2 == 0 {
            "profiles".to_owned()
        } else {
            "metrics".to_owned()
        }
    } else {
        format!("missing-{missing_seed}")
    }
}

fn generated_key(route_should_fail: bool, key_seed: u16, key_tail: u16) -> Bytes {
    Bytes::from(vec![
        if route_should_fail {
            ROUTE_FAIL_MARKER
        } else {
            ROUTE_OK_MARKER
        },
        key_seed as u8,
        key_tail as u8,
    ])
}

fn route_fails(key: &[u8]) -> bool {
    key.first().copied() == Some(ROUTE_FAIL_MARKER)
}

fn expected_put_result(space: &str, key: &[u8], mutations: &[Mutation]) -> WriteResult {
    match (space.len() + key.len() + mutations.len()) % 3 {
        0 => WriteResult::Written,
        1 => WriteResult::ConditionFailed,
        _ => WriteResult::Missing,
    }
}

fn expected_delete_result(space: &str, key: &[u8]) -> WriteResult {
    match (space.len() + key.len()) % 3 {
        0 => WriteResult::Written,
        1 => WriteResult::ConditionFailed,
        _ => WriteResult::Missing,
    }
}

fn expected_conditional_put_result(
    space: &str,
    key: &[u8],
    checks: &[Check],
    mutations: &[Mutation],
) -> WriteResult {
    match (space.len() + key.len() + checks.len() + mutations.len()) % 3 {
        0 => WriteResult::Written,
        1 => WriteResult::ConditionFailed,
        _ => WriteResult::Missing,
    }
}

fn expected_get_result(space: &str, key: &[u8]) -> Option<Record> {
    if (space.len() + key.len()) % 2 == 0 {
        Some(expected_record(space, key, i64::from(key[1])))
    } else {
        None
    }
}

fn expected_search_result(space: &str, checks: &[Check]) -> Vec<Record> {
    if checks.is_empty() {
        return vec![expected_record(space, b"search-empty", 0)];
    }

    checks
        .iter()
        .enumerate()
        .map(|(index, check)| {
            let key = format!("search-{space}-{index}");
            expected_record(
                space,
                key.as_bytes(),
                i64::from(predicate_code(check.predicate)) + i64::try_from(index).unwrap(),
            )
        })
        .collect()
}

fn expected_count_result(space: &str, checks: &[Check]) -> u64 {
    (space.len() as u64) + (checks.len() as u64) * 2
}

fn expected_delete_matching_result(space: &str, checks: &[Check]) -> u64 {
    expected_count_result(space, checks) + 1
}

fn expected_record(space: &str, key: &[u8], marker: i64) -> Record {
    Record::from_attributes(
        Bytes::copy_from_slice(key),
        vec![
            Attribute {
                name: "space".to_owned(),
                value: Value::String(space.to_owned()),
            },
            Attribute {
                name: "marker".to_owned(),
                value: Value::Int(marker),
            },
            Attribute {
                name: "key_len".to_owned(),
                value: Value::Int(i64::try_from(key.len()).unwrap()),
            },
        ],
    )
}

fn predicate_code(predicate: Predicate) -> u8 {
    match predicate {
        Predicate::Equal => 0,
        Predicate::LessThan => 1,
        Predicate::LessThanOrEqual => 2,
        Predicate::GreaterThan => 3,
        Predicate::GreaterThanOrEqual => 4,
    }
}

fn test_space(name: &str) -> Space {
    Space {
        name: name.to_owned(),
        key_attribute: "key".to_owned(),
        attributes: vec![AttributeDefinition {
            name: "value".to_owned(),
            kind: ValueKind::String,
        }],
        subspaces: vec![Subspace {
            dimensions: vec!["value".to_owned()],
        }],
        options: SpaceOptions {
            fault_tolerance: 0,
            partitions: 16,
            schema_format: SchemaFormat::HyperDexDsl,
        },
    }
}
