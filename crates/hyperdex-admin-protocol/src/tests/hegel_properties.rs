#![allow(clippy::expect_used, clippy::unwrap_used)]

use ::hegel::TestCase;
use ::hegel::generators as gs;

use super::*;

#[derive(Clone, Debug)]
struct GeneratedSpaceFixture {
    encoded_space: Vec<u8>,
    expected_space: Space,
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

fn generated_timestamp_datatype(raw: u8) -> u16 {
    match generated_time_unit(raw) {
        TimeUnit::Second => 9472,
        TimeUnit::Minute => 9474,
        TimeUnit::Hour => 9475,
        TimeUnit::Day => 9476,
        TimeUnit::Week => 9477,
        TimeUnit::Month => 9478,
    }
}

fn generated_subspace_attrs(
    attribute_count: usize,
    count_raw: u8,
    first_raw: u8,
    second_raw: u8,
) -> Vec<u16> {
    let desired = usize::from(count_raw % 3) + 1;
    let mut attrs = Vec::new();

    for raw in [
        first_raw,
        second_raw,
        first_raw.wrapping_add(second_raw),
        first_raw.wrapping_mul(3).wrapping_add(1),
    ] {
        let attr = (usize::from(raw) % attribute_count) as u16;
        if !attrs.contains(&attr) {
            attrs.push(attr);
        }
        if attrs.len() == desired {
            break;
        }
    }

    if attrs.is_empty() {
        attrs.push(0);
    }

    attrs
}

fn generated_space_fixture(
    space_seed: u16,
    fault_tolerance_raw: u8,
    partitions_raw: u8,
    attr_specs: Vec<(u8, u16)>,
    subspace_specs: Vec<(u8, u8, u8)>,
    index_specs: Vec<(u8, u8, u16)>,
) -> GeneratedSpaceFixture {
    let space_name = generated_label("space", space_seed);
    let key_attribute = generated_label("key", space_seed);
    let partitions = u32::from(partitions_raw) + 1;

    let mut attribute_names = vec![key_attribute.clone()];
    let mut attribute_datatypes = vec![HYPERDATATYPE_STRING];
    let mut expected_attributes = Vec::new();

    for (index, (kind_raw, name_seed)) in attr_specs.into_iter().enumerate() {
        match kind_raw % 6 {
            0 => {
                let name = format!("attr{space_seed}_{index}_{name_seed}");
                attribute_names.push(name.clone());
                attribute_datatypes.push(HYPERDATATYPE_STRING);
                expected_attributes.push(AttributeDefinition {
                    name,
                    kind: ValueKind::String,
                });
            }
            1 => {
                let name = format!("attr{space_seed}_{index}_{name_seed}");
                attribute_names.push(name.clone());
                attribute_datatypes.push(HYPERDATATYPE_INT64);
                expected_attributes.push(AttributeDefinition {
                    name,
                    kind: ValueKind::Int,
                });
            }
            2 => {
                let name = format!("attr{space_seed}_{index}_{name_seed}");
                attribute_names.push(name.clone());
                attribute_datatypes.push(HYPERDATATYPE_FLOAT);
                expected_attributes.push(AttributeDefinition {
                    name,
                    kind: ValueKind::Float,
                });
            }
            3 => {
                let name = format!("attr{space_seed}_{index}_{name_seed}");
                attribute_names.push(name.clone());
                attribute_datatypes.push(HYPERDATATYPE_DOCUMENT);
                expected_attributes.push(AttributeDefinition {
                    name,
                    kind: ValueKind::Document,
                });
            }
            4 => {
                let name = format!("attr{space_seed}_{index}_{name_seed}");
                let unit_raw = (name_seed as u8).wrapping_add(kind_raw);
                attribute_names.push(name.clone());
                attribute_datatypes.push(generated_timestamp_datatype(unit_raw));
                expected_attributes.push(AttributeDefinition {
                    name,
                    kind: ValueKind::Timestamp(generated_time_unit(unit_raw)),
                });
            }
            _ => {
                attribute_names.push(HYPERDEX_ATTRIBUTE_SECRET.to_owned());
                attribute_datatypes.push(HYPERDATATYPE_MACAROON_SECRET);
            }
        }
    }

    let packed_attributes = attribute_names
        .iter()
        .zip(attribute_datatypes.iter().copied())
        .map(|(name, datatype)| PackedAttribute {
            name: name.as_str(),
            datatype,
        })
        .collect::<Vec<_>>();

    let attribute_count = packed_attributes.len();
    let mut packed_subspaces = vec![packed_primary_subspace(&[0], partitions)];
    let mut expected_subspaces = Vec::new();

    for (index, (count_raw, first_raw, second_raw)) in subspace_specs.into_iter().enumerate() {
        let attrs = generated_subspace_attrs(attribute_count, count_raw, first_raw, second_raw);
        packed_subspaces.push(packed_secondary_subspace(
            32 + index as u64,
            &attrs,
            partitions,
        ));

        let dimensions = attrs
            .iter()
            .filter_map(|&attr_index| {
                let attr_index = usize::from(attr_index);
                let datatype = attribute_datatypes[attr_index];
                let name = &attribute_names[attr_index];

                if attr_index == 0 || datatype == HYPERDATATYPE_MACAROON_SECRET {
                    None
                } else {
                    Some(name.clone())
                }
            })
            .collect::<Vec<_>>();

        if !dimensions.is_empty() {
            expected_subspaces.push(Subspace { dimensions });
        }
    }

    let index_extras = index_specs
        .iter()
        .map(|&(type_raw, _, extra_seed)| {
            if type_raw % 2 == 0 {
                Vec::new()
            } else {
                format!("field_{extra_seed}").into_bytes()
            }
        })
        .collect::<Vec<_>>();

    let packed_indices = index_specs
        .into_iter()
        .enumerate()
        .map(|(index, (type_raw, attr_raw, _))| PackedIndex {
            index_type: if type_raw % 2 == 0 {
                INDEX_TYPE_NORMAL
            } else {
                INDEX_TYPE_DOCUMENT
            },
            id: 80 + index as u64,
            attr: (usize::from(attr_raw) % attribute_count) as u16,
            extra: index_extras[index].as_slice(),
        })
        .collect::<Vec<_>>();

    let encoded_space = pack_space(PackedSpace {
        id: u64::from(space_seed) + 1,
        name: space_name.as_str(),
        fault_tolerance: u64::from(fault_tolerance_raw),
        attributes: packed_attributes,
        subspaces: packed_subspaces,
        indices: packed_indices,
    });

    GeneratedSpaceFixture {
        encoded_space,
        expected_space: Space {
            name: space_name,
            key_attribute,
            attributes: expected_attributes,
            subspaces: expected_subspaces,
            options: SpaceOptions {
                fault_tolerance: u32::from(fault_tolerance_raw),
                partitions,
                schema_format: SchemaFormat::HyperDexDsl,
            },
        },
    }
}

#[::hegel::composite]
fn generated_space(tc: TestCase) -> GeneratedSpaceFixture {
    let space_seed: u16 = tc.draw(gs::integers::<u16>().max_value(127));
    let fault_tolerance_raw: u8 = tc.draw(gs::integers::<u8>().max_value(4));
    let partitions_raw: u8 = tc.draw(gs::integers::<u8>().max_value(4));
    let attr_specs: Vec<(u8, u16)> = tc.draw(
        gs::vecs(gs::tuples2(
            gs::integers::<u8>().max_value(5),
            gs::integers::<u16>().max_value(255),
        ))
        .max_size(4),
    );
    let subspace_specs: Vec<(u8, u8, u8)> = tc.draw(
        gs::vecs(gs::tuples3(
            gs::integers::<u8>().max_value(4),
            gs::integers::<u8>().max_value(31),
            gs::integers::<u8>().max_value(31),
        ))
        .max_size(3),
    );
    let index_specs: Vec<(u8, u8, u16)> = tc.draw(
        gs::vecs(gs::tuples3(
            gs::integers::<u8>().max_value(1),
            gs::integers::<u8>().max_value(31),
            gs::integers::<u16>().max_value(255),
        ))
        .max_size(3),
    );

    generated_space_fixture(
        space_seed,
        fault_tolerance_raw,
        partitions_raw,
        attr_specs,
        subspace_specs,
        index_specs,
    )
}

fn assert_request_roundtrip(
    request: &ReplicantAdminRequestMessage,
) -> ReplicantAdminRequestMessage {
    let encoded = request.encode().unwrap();
    let decoded = ReplicantAdminRequestMessage::decode(&encoded).unwrap();
    assert_eq!(decoded, *request);
    assert_eq!(decoded.nonce(), request.nonce());
    assert_eq!(decoded.encode().unwrap(), encoded);
    decoded
}

#[::hegel::test(test_cases = 60)]
fn hegel_admin_requests_round_trip_and_preserve_supported_coordinator_mapping(tc: TestCase) {
    let case: u8 = tc.draw(gs::integers::<u8>().max_value(5));

    match case {
        0 => {
            let nonce: u64 = tc.draw(gs::integers::<u64>());
            let request = ReplicantAdminRequestMessage::get_robust_params(nonce);
            let decoded = assert_request_roundtrip(&request);
            assert!(decoded.into_coordinator_request().is_err());
        }
        1 => {
            let nonce: u64 = tc.draw(gs::integers::<u64>());
            let state: u64 = tc.draw(gs::integers::<u64>());
            let request = ReplicantAdminRequestMessage::wait_until_stable(nonce, state);
            let decoded = assert_request_roundtrip(&request);
            assert_eq!(
                decoded.into_coordinator_request().unwrap(),
                CoordinatorAdminRequest::WaitUntilStable
            );
        }
        2 => {
            let nonce: u64 = tc.draw(gs::integers::<u64>());
            let state: u64 = tc.draw(gs::integers::<u64>());
            let request = ReplicantAdminRequestMessage::CondWait {
                nonce,
                object: REPLICANT_OBJECT_HYPERDEX.to_vec(),
                condition: REPLICANT_CONDITION_CONFIG.to_vec(),
                state,
            };
            let decoded = assert_request_roundtrip(&request);
            assert_eq!(
                decoded.into_coordinator_request().unwrap(),
                CoordinatorAdminRequest::ConfigGet
            );
        }
        3 => {
            let nonce: u64 = tc.draw(gs::integers::<u64>());
            let space_seed: u16 = tc.draw(gs::integers::<u16>().max_value(255));
            let space_name = generated_label("space_rm", space_seed);
            let request = ReplicantAdminRequestMessage::space_rm(nonce, space_name.clone());
            let decoded = assert_request_roundtrip(&request);
            assert_eq!(
                decoded.into_coordinator_request().unwrap(),
                CoordinatorAdminRequest::SpaceRm(space_name)
            );
        }
        4 => {
            let nonce: u64 = tc.draw(gs::integers::<u64>());
            let space_fixture = tc.draw(generated_space());
            assert_eq!(
                decode_packed_hyperdex_space(&space_fixture.encoded_space).unwrap(),
                space_fixture.expected_space
            );

            let request =
                ReplicantAdminRequestMessage::space_add(nonce, space_fixture.encoded_space.clone());
            let decoded = assert_request_roundtrip(&request);
            assert_eq!(
                decoded.into_coordinator_request().unwrap(),
                CoordinatorAdminRequest::SpaceAdd(space_fixture.expected_space)
            );
        }
        5 => {
            let nonce: u64 = tc.draw(gs::integers::<u64>());
            let command_nonce: u64 = tc.draw(gs::integers::<u64>());
            let min_slot: u64 = tc.draw(gs::integers::<u64>());
            let space_fixture = tc.draw(generated_space());
            assert_eq!(
                decode_packed_hyperdex_space(&space_fixture.encoded_space).unwrap(),
                space_fixture.expected_space
            );

            let request = ReplicantAdminRequestMessage::CallRobust {
                nonce,
                command_nonce,
                min_slot,
                object: REPLICANT_OBJECT_HYPERDEX.to_vec(),
                function: REPLICANT_FUNCTION_SPACE_ADD.to_vec(),
                input: space_fixture.encoded_space.clone(),
            };
            let decoded = assert_request_roundtrip(&request);
            assert_eq!(
                decoded.into_coordinator_request().unwrap(),
                CoordinatorAdminRequest::SpaceAdd(space_fixture.expected_space)
            );
        }
        _ => unreachable!("case selector is bounded to 0..=5"),
    }
}
