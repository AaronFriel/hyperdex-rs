#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::{BTreeMap, BTreeSet};

use ::hegel::{TestCase, generators as gs};
use data_model::{
    AttributeDefinition, SchemaFormat, Space, SpaceName, SpaceOptions, Subspace, TimeUnit,
    ValueKind,
};
use placement_core::ClusterLayout;

use super::*;

#[derive(Clone, Debug)]
enum GeneratedOperation {
    CreateSpace(Space),
    DropSpace(String),
    RegisterDaemon(ClusterNode),
    ReplaceDaemons(Vec<ClusterNode>),
}

#[derive(Clone, Debug)]
struct CatalogModel {
    spaces: BTreeMap<SpaceName, Space>,
    nodes: BTreeMap<NodeId, ClusterNode>,
    replicas: usize,
}

impl CatalogModel {
    fn new(nodes: Vec<ClusterNode>, replicas: usize) -> Self {
        Self {
            spaces: BTreeMap::new(),
            nodes: nodes.into_iter().map(|node| (node.id, node)).collect(),
            replicas,
        }
    }

    fn create_space(&mut self, space: Space) -> bool {
        if self.spaces.contains_key(&space.name) {
            false
        } else {
            self.spaces.insert(space.name.clone(), space);
            true
        }
    }

    fn drop_space(&mut self, name: &str) {
        self.spaces.remove(name);
    }

    fn register_daemon(&mut self, node: ClusterNode) -> bool {
        match self.nodes.insert(node.id, node.clone()) {
            Some(existing) => existing != node,
            None => true,
        }
    }

    fn replace_daemons(&mut self, nodes: Vec<ClusterNode>) -> bool {
        let next = nodes
            .into_iter()
            .map(|node| (node.id, node))
            .collect::<BTreeMap<_, _>>();
        let changed = self.nodes != next;
        self.nodes = next;
        changed
    }

    fn layout(&self) -> ClusterLayout {
        ClusterLayout {
            replicas: self.replicas,
            nodes: self.nodes.keys().copied().collect(),
        }
    }
}

impl GeneratedOperation {
    fn tracked_space_name(&self) -> Option<&str> {
        match self {
            Self::CreateSpace(space) => Some(&space.name),
            Self::DropSpace(name) => Some(name),
            Self::RegisterDaemon(_) | Self::ReplaceDaemons(_) => None,
        }
    }
}

fn generated_label(prefix: &str, seed: u16) -> String {
    format!("{prefix}_{seed}")
}

fn generated_time_unit(raw: u8) -> TimeUnit {
    match raw % 6 {
        0 => TimeUnit::Second,
        1 => TimeUnit::Minute,
        2 => TimeUnit::Hour,
        3 => TimeUnit::Day,
        4 => TimeUnit::Week,
        _ => TimeUnit::Month,
    }
}

fn generated_leaf_kind(raw: u8) -> ValueKind {
    match raw % 7 {
        0 => ValueKind::Bool,
        1 => ValueKind::Int,
        2 => ValueKind::Float,
        3 => ValueKind::Bytes,
        4 => ValueKind::String,
        5 => ValueKind::Document,
        _ => ValueKind::Timestamp(generated_time_unit(raw)),
    }
}

fn generated_container_key_kind(raw: u8) -> ValueKind {
    match raw % 3 {
        0 => ValueKind::Bytes,
        1 => ValueKind::String,
        _ => ValueKind::Int,
    }
}

fn generated_value_kind(raw: u8) -> ValueKind {
    match raw % 10 {
        0..=6 => generated_leaf_kind(raw),
        7 => ValueKind::List(Box::new(generated_leaf_kind(raw.wrapping_add(1)))),
        8 => ValueKind::Set(Box::new(generated_leaf_kind(raw.wrapping_add(2)))),
        _ => ValueKind::Map {
            key: Box::new(generated_container_key_kind(raw.wrapping_add(3))),
            value: Box::new(generated_leaf_kind(raw.wrapping_add(4))),
        },
    }
}

fn generated_subspace_dimensions(
    attribute_names: &[String],
    count_raw: u8,
    first_raw: u8,
    second_raw: u8,
) -> Option<Vec<String>> {
    if attribute_names.is_empty() {
        return None;
    }

    let desired = usize::from(count_raw % 3) + 1;
    let mut dimensions = Vec::new();
    for raw in [
        first_raw,
        second_raw,
        first_raw.wrapping_add(second_raw),
        first_raw.wrapping_mul(3).wrapping_add(1),
    ] {
        let dimension = attribute_names[usize::from(raw) % attribute_names.len()].clone();
        if !dimensions.contains(&dimension) {
            dimensions.push(dimension);
        }
        if dimensions.len() == desired {
            break;
        }
    }

    Some(dimensions)
}

#[::hegel::composite]
fn generated_space(tc: TestCase) -> Space {
    let seed: u16 = tc.draw(gs::integers::<u16>().max_value(63));
    let fault_tolerance_raw: u8 = tc.draw(gs::integers::<u8>().max_value(3));
    let partitions_raw: u8 = tc.draw(gs::integers::<u8>().max_value(7));
    let attribute_specs: Vec<(u8, u16)> = tc.draw(
        gs::vecs(gs::tuples2(
            gs::integers::<u8>().max_value(9),
            gs::integers::<u16>().max_value(255),
        ))
        .max_size(4),
    );
    let subspace_specs: Vec<(u8, u8, u8)> = tc.draw(
        gs::vecs(gs::tuples3(
            gs::integers::<u8>().max_value(3),
            gs::integers::<u8>().max_value(15),
            gs::integers::<u8>().max_value(15),
        ))
        .max_size(3),
    );

    let name = generated_label("space", seed);
    let key_attribute = generated_label("key", seed);
    let attributes = attribute_specs
        .into_iter()
        .enumerate()
        .map(|(index, (kind_raw, name_seed))| AttributeDefinition {
            name: format!("attr_{seed}_{index}_{name_seed}"),
            kind: generated_value_kind(kind_raw),
        })
        .collect::<Vec<_>>();
    let attribute_names = attributes
        .iter()
        .map(|attribute| attribute.name.clone())
        .collect::<Vec<_>>();
    let subspaces = subspace_specs
        .into_iter()
        .filter_map(|(count_raw, first_raw, second_raw)| {
            generated_subspace_dimensions(&attribute_names, count_raw, first_raw, second_raw)
                .map(|dimensions| Subspace { dimensions })
        })
        .collect();

    Space {
        name,
        key_attribute,
        attributes,
        subspaces,
        options: SpaceOptions {
            fault_tolerance: u32::from(fault_tolerance_raw),
            partitions: u32::from(partitions_raw) + 1,
            schema_format: SchemaFormat::HyperDexDsl,
        },
    }
}

#[::hegel::composite]
fn generated_node(tc: TestCase) -> ClusterNode {
    let id_raw: u8 = tc.draw(gs::integers::<u8>().max_value(7));
    let host_seed: u16 = tc.draw(gs::integers::<u16>().max_value(255));
    let control_port_raw: u16 = tc.draw(gs::integers::<u16>().max_value(4095));
    let data_port_raw: u16 = tc.draw(gs::integers::<u16>().max_value(4095));

    ClusterNode {
        id: u64::from(id_raw) + 1,
        host: format!("node-{host_seed}"),
        control_port: 10_000 + control_port_raw,
        data_port: 20_000 + data_port_raw,
    }
}

#[::hegel::composite]
fn generated_operation(tc: TestCase) -> GeneratedOperation {
    match tc.draw(gs::integers::<u8>().max_value(3)) {
        0 => GeneratedOperation::CreateSpace(tc.draw(generated_space())),
        1 => {
            let seed: u16 = tc.draw(gs::integers::<u16>().max_value(63));
            GeneratedOperation::DropSpace(generated_label("space", seed))
        }
        2 => GeneratedOperation::RegisterDaemon(tc.draw(generated_node())),
        _ => GeneratedOperation::ReplaceDaemons(tc.draw(gs::vecs(generated_node()).max_size(5))),
    }
}

fn assert_catalog_matches_model(
    catalog: &InMemoryCatalog,
    model: &CatalogModel,
    tracked_space_names: &BTreeSet<String>,
) {
    let expected_spaces = model.spaces.keys().cloned().collect::<Vec<_>>();
    assert_eq!(catalog.list_spaces().unwrap(), expected_spaces);

    for name in tracked_space_names {
        assert_eq!(
            catalog.get_space(name).unwrap(),
            model.spaces.get(name).cloned()
        );
    }

    assert_eq!(catalog.layout().unwrap(), model.layout());
}

#[::hegel::test(test_cases = 40)]
fn hegel_in_memory_catalog_preserves_space_and_daemon_state_model(tc: TestCase) {
    let replicas_raw: u8 = tc.draw(gs::integers::<u8>().max_value(4));
    let initial_nodes: Vec<ClusterNode> = tc.draw(gs::vecs(generated_node()).max_size(4));
    let operations: Vec<GeneratedOperation> =
        tc.draw(gs::vecs(generated_operation()).min_size(1).max_size(24));

    let catalog = InMemoryCatalog::new(initial_nodes.clone(), usize::from(replicas_raw));
    let mut model = CatalogModel::new(initial_nodes, usize::from(replicas_raw));
    let mut tracked_space_names = BTreeSet::from([String::from("space_missing_probe")]);

    for operation in &operations {
        if let Some(name) = operation.tracked_space_name() {
            tracked_space_names.insert(name.to_owned());
        }
    }

    assert_catalog_matches_model(&catalog, &model, &tracked_space_names);

    for operation in operations {
        match operation {
            GeneratedOperation::CreateSpace(space) => {
                let expected_ok = model.create_space(space.clone());
                assert_eq!(catalog.create_space(space).is_ok(), expected_ok);
            }
            GeneratedOperation::DropSpace(name) => {
                model.drop_space(&name);
                catalog.drop_space(&name).unwrap();
            }
            GeneratedOperation::RegisterDaemon(node) => {
                let expected_changed = model.register_daemon(node.clone());
                assert_eq!(catalog.register_daemon(node).unwrap(), expected_changed);
            }
            GeneratedOperation::ReplaceDaemons(nodes) => {
                let expected_changed = model.replace_daemons(nodes.clone());
                assert_eq!(catalog.replace_daemons(nodes).unwrap(), expected_changed);
            }
        }

        assert_catalog_matches_model(&catalog, &model, &tracked_space_names);
    }
}
