use super::*;
use data_model::{AttributeDefinition, SchemaFormat, SpaceOptions, Subspace, TimeUnit, ValueKind};

#[derive(Clone, Debug)]
struct PackedAttribute<'a> {
    name: &'a str,
    datatype: u16,
}

#[derive(Clone, Debug)]
struct PackedReplica {
    server_id: u64,
    virtual_server_id: u64,
}

#[derive(Clone, Debug)]
struct PackedRegion {
    id: u64,
    bounds: Vec<(u64, u64)>,
    replicas: Vec<PackedReplica>,
}

#[derive(Clone, Debug)]
struct PackedSubspace {
    id: u64,
    attrs: Vec<u16>,
    regions: Vec<PackedRegion>,
}

#[derive(Clone, Debug)]
struct PackedIndex<'a> {
    index_type: u8,
    id: u64,
    attr: u16,
    extra: &'a [u8],
}

#[derive(Clone, Debug)]
struct PackedSpace<'a> {
    id: u64,
    name: &'a str,
    fault_tolerance: u64,
    attributes: Vec<PackedAttribute<'a>>,
    subspaces: Vec<PackedSubspace>,
    indices: Vec<PackedIndex<'a>>,
}

#[test]
fn busybee_frame_round_trip() {
    let frame = BusyBeeFrame::identify(vec![0_u8; 16]);
    let encoded = frame.encode().unwrap();

    assert_eq!(BusyBeeFrame::decode(&encoded).unwrap(), frame);
}

#[test]
fn varint_slice_round_trip() {
    let payload = b"hyperdex-admin";
    let encoded = encode_varint_slice(payload);
    let (decoded, consumed) = decode_varint_slice(&encoded).unwrap();

    assert_eq!(decoded, payload);
    assert_eq!(consumed, encoded.len());
}

#[test]
fn captured_bootstrap_request_matches_original_tool_bytes() {
    let encoded = ReplicantAdminRequestMessage::bootstrap_request();

    assert_eq!(encoded, CAPTURED_INITIAL_CONFIG_FOLLOW_REQUEST);

    let frames = BusyBeeFrame::decode_stream(&encoded).unwrap();
    assert_eq!(
        frames,
        vec![
            BusyBeeFrame::identify(vec![0_u8; 16]),
            BusyBeeFrame::new(vec![0x1c])
        ]
    );
    assert_eq!(BusyBeeFrame::encode_stream(&frames).unwrap(), encoded);
}

#[test]
fn wait_until_stable_message_round_trip() {
    let message = ReplicantAdminRequestMessage::wait_until_stable(7, 11);
    let encoded = message.encode().unwrap();
    let decoded = ReplicantAdminRequestMessage::decode(&encoded).unwrap();

    assert_eq!(decoded, message);
}

#[test]
fn get_robust_params_message_round_trip() {
    let message = ReplicantAdminRequestMessage::get_robust_params(13);
    let encoded = message.encode().unwrap();
    let decoded = ReplicantAdminRequestMessage::decode(&encoded).unwrap();

    assert_eq!(decoded, message);
}

#[test]
fn space_rm_message_round_trip() {
    let message = ReplicantAdminRequestMessage::space_rm(9, "profiles".to_owned());
    let encoded = message.encode().unwrap();
    let decoded = ReplicantAdminRequestMessage::decode(&encoded).unwrap();

    assert_eq!(decoded, message);
}

#[test]
fn packed_space_decoder_translates_original_hyperdex_layout() {
    let encoded_space = encode_test_space_payload();

    let decoded = decode_packed_hyperdex_space(&encoded_space).unwrap();

    assert_eq!(
        decoded,
        Space {
            name: "profiles".to_owned(),
            key_attribute: "username".to_owned(),
            attributes: vec![
                AttributeDefinition {
                    name: "first".to_owned(),
                    kind: ValueKind::String,
                },
                AttributeDefinition {
                    name: "profile_views".to_owned(),
                    kind: ValueKind::Int,
                },
                AttributeDefinition {
                    name: "upvotes".to_owned(),
                    kind: ValueKind::Map {
                        key: Box::new(ValueKind::String),
                        value: Box::new(ValueKind::Int),
                    },
                },
                AttributeDefinition {
                    name: "created".to_owned(),
                    kind: ValueKind::Timestamp(TimeUnit::Day),
                },
            ],
            subspaces: vec![Subspace {
                dimensions: vec!["profile_views".to_owned(), "upvotes".to_owned()],
            }],
            options: SpaceOptions {
                fault_tolerance: 2,
                partitions: 2,
                schema_format: SchemaFormat::HyperDexDsl,
            },
        }
    );
}

#[test]
fn space_add_request_maps_through_packed_space_decoder() {
    let request = ReplicantAdminRequestMessage::space_add(41, encode_test_space_payload());

    let mapped = request.into_coordinator_request().unwrap();

    assert_eq!(
        mapped,
        CoordinatorAdminRequest::SpaceAdd(Space {
            name: "profiles".to_owned(),
            key_attribute: "username".to_owned(),
            attributes: vec![
                AttributeDefinition {
                    name: "first".to_owned(),
                    kind: ValueKind::String,
                },
                AttributeDefinition {
                    name: "profile_views".to_owned(),
                    kind: ValueKind::Int,
                },
                AttributeDefinition {
                    name: "upvotes".to_owned(),
                    kind: ValueKind::Map {
                        key: Box::new(ValueKind::String),
                        value: Box::new(ValueKind::Int),
                    },
                },
                AttributeDefinition {
                    name: "created".to_owned(),
                    kind: ValueKind::Timestamp(TimeUnit::Day),
                },
            ],
            subspaces: vec![Subspace {
                dimensions: vec!["profile_views".to_owned(), "upvotes".to_owned()],
            }],
            options: SpaceOptions {
                fault_tolerance: 2,
                partitions: 2,
                schema_format: SchemaFormat::HyperDexDsl,
            },
        })
    );
}

#[test]
fn packed_space_decoder_rejects_secret_attribute_with_wrong_name() {
    let encoded_space = pack_space(PackedSpace {
        id: 1,
        name: "profiles",
        fault_tolerance: 0,
        attributes: vec![
            PackedAttribute {
                name: "username",
                datatype: HYPERDATATYPE_STRING,
            },
            PackedAttribute {
                name: "api_secret",
                datatype: HYPERDATATYPE_MACAROON_SECRET,
            },
        ],
        subspaces: vec![packed_primary_subspace(&[0], 1)],
        indices: vec![],
    });

    let err = decode_packed_hyperdex_space(&encoded_space)
        .unwrap_err()
        .to_string();

    assert!(err.contains("authorization attribute name `api_secret`, expected `__secret`"));
}

#[test]
fn packed_space_decoder_rejects_secret_key_attribute() {
    let encoded_space = pack_space(PackedSpace {
        id: 1,
        name: "profiles",
        fault_tolerance: 0,
        attributes: vec![PackedAttribute {
            name: HYPERDEX_ATTRIBUTE_SECRET,
            datatype: HYPERDATATYPE_MACAROON_SECRET,
        }],
        subspaces: vec![packed_primary_subspace(&[0], 1)],
        indices: vec![],
    });

    let err = decode_packed_hyperdex_space(&encoded_space)
        .unwrap_err()
        .to_string();

    assert!(err.contains("key attribute cannot be the authorization secret"));
}

#[test]
fn packed_space_decoder_rejects_inconsistent_partition_counts() {
    let encoded_space = pack_space(PackedSpace {
        id: 1,
        name: "profiles",
        fault_tolerance: 0,
        attributes: vec![
            PackedAttribute {
                name: "username",
                datatype: HYPERDATATYPE_STRING,
            },
            PackedAttribute {
                name: "profile_views",
                datatype: HYPERDATATYPE_INT64,
            },
        ],
        subspaces: vec![
            packed_primary_subspace(&[0], 2),
            packed_secondary_subspace(2, &[1], 3),
        ],
        indices: vec![],
    });

    let err = decode_packed_hyperdex_space(&encoded_space)
        .unwrap_err()
        .to_string();

    assert!(err.contains("subspaces disagree on partition count: 2 vs 3"));
}

#[test]
fn packed_space_decoder_rejects_unknown_index_type() {
    let mut packed = test_packed_space();
    packed.indices[0].index_type = 9;

    let err = decode_packed_hyperdex_space(&pack_space(packed))
        .unwrap_err()
        .to_string();

    assert!(err.contains("uses unknown index type 9"));
}

#[test]
fn packed_space_decoder_rejects_index_attribute_out_of_range() {
    let mut packed = test_packed_space();
    packed.indices[0].attr = 99;

    let err = decode_packed_hyperdex_space(&pack_space(packed))
        .unwrap_err()
        .to_string();

    assert!(err.contains("index references attribute index 99, but only 6 attributes were decoded"));
}

#[test]
fn packed_space_decoder_reports_truncated_region_replicas() {
    let mut packed = test_packed_space();
    packed.indices.clear();
    let mut encoded_space = pack_space(packed);
    encoded_space.pop();

    let err = decode_packed_hyperdex_space(&encoded_space)
        .unwrap_err()
        .to_string();

    assert!(err.contains("region replicas is truncated"));
}

#[test]
fn packed_space_decoder_reports_truncated_index_extra_payload() {
    let mut packed = test_packed_space();
    packed.indices[1].extra = b"comments.author";
    let mut encoded_space = pack_space(packed);
    encoded_space.pop();

    let err = decode_packed_hyperdex_space(&encoded_space)
        .unwrap_err()
        .to_string();

    assert!(err.contains("index extra payload is truncated"));
}

#[test]
fn stable_wait_and_space_rm_map_to_coordinator_requests() {
    assert_eq!(
        ReplicantAdminRequestMessage::wait_until_stable(7, 11)
            .into_coordinator_request()
            .unwrap(),
        CoordinatorAdminRequest::WaitUntilStable
    );
    assert_eq!(
        ReplicantAdminRequestMessage::space_rm(9, "profiles".to_owned())
            .into_coordinator_request()
            .unwrap(),
        CoordinatorAdminRequest::SpaceRm("profiles".to_owned())
    );
}

#[test]
fn call_completion_response_decodes() {
    let response = ReplicantCallCompletion {
        nonce: 14,
        status: ReplicantReturnCode::Success,
        output: CoordinatorReturnCode::Success.encode().to_vec(),
    };
    let encoded = response.encode();

    assert_eq!(ReplicantCallCompletion::decode(&encoded).unwrap(), response);
}

#[test]
fn cond_wait_completion_response_decodes() {
    let response = ReplicantConditionCompletion {
        nonce: 18,
        status: ReplicantReturnCode::Success,
        state: 4,
        data: vec![0xde, 0xad, 0xbe, 0xef],
    };
    let encoded = response.encode();

    assert_eq!(
        ReplicantConditionCompletion::decode(&encoded).unwrap(),
        response
    );
}

#[test]
fn bootstrap_response_round_trips() {
    let response = ReplicantBootstrapResponse {
        server: ReplicantBootstrapServer {
            id: 1,
            address: "127.0.0.1:1982".parse().unwrap(),
        },
        configuration: ReplicantBootstrapConfiguration {
            cluster_id: 1,
            version: 1,
            first_slot: 1,
            servers: vec![ReplicantBootstrapServer {
                id: 1,
                address: "127.0.0.1:1982".parse().unwrap(),
            }],
        },
    };
    let encoded = response.encode();

    assert_eq!(
        ReplicantBootstrapResponse::decode(&encoded).unwrap(),
        response
    );
}

#[test]
fn coordinator_return_codes_round_trip_through_wire_bytes() {
    let codes = [
        CoordinatorReturnCode::Success,
        CoordinatorReturnCode::Malformed,
        CoordinatorReturnCode::Duplicate,
        CoordinatorReturnCode::NotFound,
        CoordinatorReturnCode::Uninitialized,
        CoordinatorReturnCode::NoCanDo,
    ];

    for code in codes {
        assert_eq!(CoordinatorReturnCode::decode(&code.encode()).unwrap(), code);
    }
}

#[test]
fn coordinator_return_codes_map_to_hyperdex_admin_statuses() {
    assert_eq!(
        CoordinatorReturnCode::Success.legacy_admin_status(),
        LegacyAdminReturnCode::Success
    );
    assert_eq!(
        CoordinatorReturnCode::Duplicate.legacy_admin_status(),
        LegacyAdminReturnCode::Duplicate
    );
    assert_eq!(
        CoordinatorReturnCode::NotFound.legacy_admin_status(),
        LegacyAdminReturnCode::NotFound
    );
    assert_eq!(
        CoordinatorReturnCode::Uninitialized.legacy_admin_status(),
        LegacyAdminReturnCode::CoordFail
    );
    assert_eq!(
        CoordinatorReturnCode::NoCanDo.legacy_admin_status(),
        LegacyAdminReturnCode::CoordFail
    );
    assert_eq!(
        CoordinatorReturnCode::Malformed.legacy_admin_status(),
        LegacyAdminReturnCode::Internal
    );
}

#[test]
fn coordinator_admin_requests_expose_hyperdex_method_names() {
    let space = Space {
        name: "profiles".to_owned(),
        key_attribute: "username".to_owned(),
        attributes: vec![AttributeDefinition {
            name: "first".to_owned(),
            kind: ValueKind::String,
        }],
        subspaces: vec![Subspace {
            dimensions: vec!["username".to_owned()],
        }],
        options: SpaceOptions {
            fault_tolerance: 0,
            partitions: 64,
            schema_format: SchemaFormat::HyperDexDsl,
        },
    };

    assert_eq!(
        CoordinatorAdminRequest::DaemonRegister(ClusterNode {
            id: 9,
            host: "127.0.0.1".to_owned(),
            control_port: 1982,
            data_port: 2012,
        })
        .method_name(),
        "daemon_register"
    );
    assert_eq!(
        CoordinatorAdminRequest::SpaceAdd(space).method_name(),
        "space_add"
    );
    assert_eq!(
        CoordinatorAdminRequest::SpaceRm("profiles".to_owned()).method_name(),
        "space_rm"
    );
    assert_eq!(
        CoordinatorAdminRequest::WaitUntilStable.method_name(),
        "wait_until_stable"
    );
    assert_eq!(
        CoordinatorAdminRequest::ConfigGet.method_name(),
        "config_get"
    );
}

fn encode_test_space_payload() -> Vec<u8> {
    pack_space(test_packed_space())
}

fn test_packed_space<'a>() -> PackedSpace<'a> {
    PackedSpace {
        id: 17,
        name: "profiles",
        fault_tolerance: 2,
        attributes: vec![
            PackedAttribute {
                name: "username",
                datatype: HYPERDATATYPE_STRING,
            },
            PackedAttribute {
                name: "first",
                datatype: HYPERDATATYPE_STRING,
            },
            PackedAttribute {
                name: "profile_views",
                datatype: HYPERDATATYPE_INT64,
            },
            PackedAttribute {
                name: "upvotes",
                datatype: 9418,
            },
            PackedAttribute {
                name: "created",
                datatype: 9476,
            },
            PackedAttribute {
                name: HYPERDEX_ATTRIBUTE_SECRET,
                datatype: HYPERDATATYPE_MACAROON_SECRET,
            },
        ],
        subspaces: vec![
            packed_primary_subspace(&[0], 2),
            packed_secondary_subspace(32, &[2, 3], 2),
        ],
        indices: vec![
            PackedIndex {
                index_type: INDEX_TYPE_NORMAL,
                id: 41,
                attr: 2,
                extra: b"",
            },
            PackedIndex {
                index_type: INDEX_TYPE_DOCUMENT,
                id: 42,
                attr: 3,
                extra: b"comments.author",
            },
        ],
    }
}

fn packed_primary_subspace(attrs: &[u16], partitions: u32) -> PackedSubspace {
    packed_secondary_subspace(31, attrs, partitions)
}

fn packed_secondary_subspace(id: u64, attrs: &[u16], partitions: u32) -> PackedSubspace {
    let bounds = vec![(0_u64, 100_u64); attrs.len().max(1)];
    let mut regions = Vec::new();
    for partition in 0..partitions {
        let base = partition as u64 + 1;
        regions.push(PackedRegion {
            id: id * 10 + partition as u64,
            bounds: bounds.clone(),
            replicas: vec![PackedReplica {
                server_id: base,
                virtual_server_id: base * 10,
            }],
        });
    }
    PackedSubspace {
        id,
        attrs: attrs.to_vec(),
        regions,
    }
}

fn pack_space(space: PackedSpace<'_>) -> Vec<u8> {
    let mut out = Vec::new();
    encode_u64(&mut out, space.id);
    encode_slice(&mut out, space.name.as_bytes());
    encode_u64(&mut out, space.fault_tolerance);
    encode_u16(&mut out, space.attributes.len() as u16);
    encode_u16(&mut out, space.subspaces.len() as u16);
    encode_u16(&mut out, space.indices.len() as u16);

    for attribute in space.attributes {
        encode_slice(&mut out, attribute.name.as_bytes());
        encode_u16(&mut out, attribute.datatype);
    }

    for subspace in space.subspaces {
        encode_u64(&mut out, subspace.id);
        encode_u16(&mut out, subspace.attrs.len() as u16);
        encode_u32(&mut out, subspace.regions.len() as u32);

        for attr in subspace.attrs {
            encode_u16(&mut out, attr);
        }

        for region in subspace.regions {
            encode_u64(&mut out, region.id);
            encode_u16(&mut out, region.bounds.len() as u16);
            encode_u8(&mut out, region.replicas.len() as u8);

            for (lower, upper) in region.bounds {
                encode_u64(&mut out, lower);
                encode_u64(&mut out, upper);
            }

            for replica in region.replicas {
                encode_u64(&mut out, replica.server_id);
                encode_u64(&mut out, replica.virtual_server_id);
            }
        }
    }

    for index in space.indices {
        encode_u8(&mut out, index.index_type);
        encode_u64(&mut out, index.id);
        encode_u16(&mut out, index.attr);
        encode_slice(&mut out, index.extra);
    }

    out
}

fn encode_slice(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&encode_varint_slice(bytes));
}

fn encode_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn encode_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn encode_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn encode_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}
