use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use ::hegel::{generators, Hegel, Settings, TestCase};
use bytes::Bytes;
use data_model::{Attribute, Check, Mutation, NumericOp, Predicate, Value};
use storage_core::{StorageEngine, WriteResult};

use crate::MemoryEngine;

static HEGEL_SERVER_COMMAND: OnceLock<String> = OnceLock::new();
static HEGEL_ENV_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, PartialEq, Eq)]
struct ModelRecord {
    count: i64,
    name: String,
    score: i64,
}

fn ensure_hegel_server_command() -> String {
    HEGEL_SERVER_COMMAND
        .get_or_init(|| {
            let root = std::env::temp_dir().join(format!(
                "hyperdex-rs-hegel-core-0.2.3-{}",
                std::process::id()
            ));
            let venv_dir = root.join("venv");
            let hegel = venv_dir.join("bin/hegel");
            let pyvenv_cfg = venv_dir.join("pyvenv.cfg");

            if hegel.is_file() && pyvenv_cfg.is_file() {
                return hegel.to_str().expect("hegel path must be utf-8").to_owned();
            }

            if venv_dir.exists() && !pyvenv_cfg.is_file() {
                std::fs::remove_dir_all(&venv_dir).expect("remove invalid hegel venv dir");
            }

            std::fs::create_dir_all(&root).expect("create hegel temp dir");

            let status = std::process::Command::new("uv")
                .args(["venv", "--clear"])
                .arg(&venv_dir)
                .status()
                .expect("run uv venv");
            assert!(status.success(), "uv venv failed for {:?}", venv_dir);

            let python = venv_dir.join("bin/python");
            let status = std::process::Command::new("uv")
                .args(["pip", "install", "--python"])
                .arg(&python)
                .arg("hegel-core==0.2.3")
                .status()
                .expect("run uv pip install");
            assert!(status.success(), "uv pip install failed for {:?}", python);

            assert!(hegel.is_file(), "missing hegel binary at {:?}", hegel);
            hegel.to_str().expect("hegel path must be utf-8").to_owned()
        })
        .clone()
}

fn key_name(key_id: u8) -> String {
    format!("k{key_id}")
}

fn count_value(record: &data_model::Record) -> i64 {
    match record.attributes.get("count") {
        Some(Value::Int(value)) => *value,
        other => panic!("expected int count attribute, got {:?}", other),
    }
}

fn name_value(record: &data_model::Record) -> String {
    match record.attributes.get("name") {
        Some(Value::String(value)) => value.clone(),
        other => panic!("expected string name attribute, got {:?}", other),
    }
}

fn score_value(record: &data_model::Record) -> i64 {
    match record.attributes.get("scores") {
        Some(Value::Map(map)) => match map.get(&Value::String("hot".to_owned())) {
            Some(Value::Int(value)) => *value,
            other => panic!("expected int hot score entry, got {:?}", other),
        },
        other => panic!("expected map scores attribute, got {:?}", other),
    }
}

fn search_check(threshold: i64) -> [Check; 1] {
    [Check {
        attribute: "count".to_owned(),
        predicate: Predicate::GreaterThanOrEqual,
        value: Value::Int(threshold),
    }]
}

fn search_summary(engine: &MemoryEngine, threshold: i64) -> Vec<(Vec<u8>, i64, String, i64)> {
    let mut summary = engine
        .search("profiles", &search_check(threshold))
        .unwrap()
        .into_iter()
        .map(|record| {
            (
                record.key.to_vec(),
                count_value(&record),
                name_value(&record),
                score_value(&record),
            )
        })
        .collect::<Vec<_>>();
    summary.sort();
    summary
}

fn model_search_summary(
    model: &BTreeMap<String, ModelRecord>,
    threshold: i64,
) -> Vec<(Vec<u8>, i64, String, i64)> {
    model
        .iter()
        .filter(|(_, record)| record.count >= threshold)
        .map(|(key, record)| {
            (
                key.as_bytes().to_vec(),
                record.count,
                record.name.clone(),
                record.score,
            )
        })
        .collect()
}

fn assert_engine_matches_model(
    engine: &MemoryEngine,
    model: &BTreeMap<String, ModelRecord>,
    known_keys: &[String],
    threshold: i64,
) {
    for key in known_keys {
        let actual = engine
            .get("profiles", key.as_bytes())
            .unwrap()
            .map(|record| {
                (
                    count_value(&record),
                    name_value(&record),
                    score_value(&record),
                )
            });
        let expected = model
            .get(key)
            .map(|record| (record.count, record.name.clone(), record.score));
        assert_eq!(actual, expected, "get mismatch for key {key}");
    }

    let expected_search = model_search_summary(model, threshold);
    let actual_search = search_summary(engine, threshold);
    assert_eq!(actual_search, expected_search, "search mismatch");
    assert_eq!(
        engine.count("profiles", &search_check(threshold)).unwrap(),
        expected_search.len() as u64,
        "count mismatch"
    );
}

#[test]
fn hegel_memory_engine_preserves_conditional_and_delete_matching_model() {
    let _guard = HEGEL_ENV_LOCK.lock().unwrap();
    let hegel_server_command = ensure_hegel_server_command();
    unsafe {
        std::env::set_var("HEGEL_SERVER_COMMAND", &hegel_server_command);
    }

    Hegel::new(|tc: TestCase| {
        let ops: Vec<(u8, u8, u8, u8, u8)> = tc.draw(
            generators::vecs(generators::tuples5(
                generators::integers::<u8>().max_value(6),
                generators::integers::<u8>().max_value(5),
                generators::integers::<u8>().max_value(18),
                generators::integers::<u8>().max_value(18),
                generators::integers::<u8>().max_value(18),
            ))
            .min_size(1)
            .max_size(35),
        );

        let engine = MemoryEngine::new();
        engine.create_space("profiles".to_owned()).unwrap();

        let known_keys = (0..=5).map(key_name).collect::<Vec<_>>();
        let mut model = BTreeMap::<String, ModelRecord>::new();

        for (kind, key_id, raw_a, raw_b, raw_threshold) in ops {
            let key = key_name(key_id);
            let threshold = i64::from(raw_threshold);
            let base_count = i64::from(raw_a);
            let delta = i64::from(raw_b) - 9;
            let expected_count = i64::from(raw_b);
            let expected_score = i64::from(raw_a);

            match kind {
                0 => {
                    let result = engine.put(
                        "profiles",
                        Bytes::from(key.clone()),
                        &[
                            Mutation::Set(Attribute {
                                name: "count".to_owned(),
                                value: Value::Int(base_count),
                            }),
                            Mutation::Set(Attribute {
                                name: "name".to_owned(),
                                value: Value::String(format!("put-{raw_a}")),
                            }),
                            Mutation::MapSet {
                                attribute: "scores".to_owned(),
                                map_key: Value::String("hot".to_owned()),
                                value: Value::Int(base_count),
                            },
                        ],
                    );
                    assert_eq!(result.unwrap(), WriteResult::Written);
                    model.insert(
                        key.clone(),
                        ModelRecord {
                            count: base_count,
                            name: format!("put-{raw_a}"),
                            score: base_count,
                        },
                    );
                }
                1 => {
                    let result = engine.conditional_put(
                        "profiles",
                        Bytes::from(key.clone()),
                        &[Check {
                            attribute: "count".to_owned(),
                            predicate: Predicate::Equal,
                            value: Value::Int(expected_count),
                        }],
                        &[
                            Mutation::Numeric {
                                attribute: "count".to_owned(),
                                op: NumericOp::Add,
                                operand: delta,
                            },
                            Mutation::MapNumeric {
                                attribute: "scores".to_owned(),
                                map_key: Value::String("hot".to_owned()),
                                op: NumericOp::Add,
                                operand: delta,
                            },
                            Mutation::Set(Attribute {
                                name: "name".to_owned(),
                                value: Value::String(format!("cond-{raw_b}")),
                            }),
                        ],
                    );

                    let expected = match model.get_mut(&key) {
                        Some(record) if record.count == expected_count => {
                            record.count += delta;
                            record.score += delta;
                            record.name = format!("cond-{raw_b}");
                            WriteResult::Written
                        }
                        _ => WriteResult::ConditionFailed,
                    };
                    assert_eq!(result.unwrap(), expected);
                }
                2 => {
                    let result = engine.delete("profiles", key.as_bytes());
                    let expected = if model.remove(&key).is_some() {
                        WriteResult::Written
                    } else {
                        WriteResult::Missing
                    };
                    assert_eq!(result.unwrap(), expected);
                }
                3 => {
                    let deleted = engine
                        .delete_matching("profiles", &search_check(threshold))
                        .unwrap();
                    let doomed = model
                        .iter()
                        .filter(|(_, record)| record.count >= threshold)
                        .map(|(key, _)| key.clone())
                        .collect::<Vec<_>>();
                    for doomed_key in &doomed {
                        model.remove(doomed_key);
                    }
                    assert_eq!(deleted, doomed.len() as u64);
                }
                4 => {
                    let result = engine.conditional_put(
                        "profiles",
                        Bytes::from(key.clone()),
                        &[Check {
                            attribute: "count".to_owned(),
                            predicate: Predicate::GreaterThanOrEqual,
                            value: Value::Int(0),
                        }],
                        &[Mutation::MapSet {
                            attribute: "scores".to_owned(),
                            map_key: Value::String("hot".to_owned()),
                            value: Value::Int(expected_score),
                        }],
                    );
                    let expected = match model.get_mut(&key) {
                        Some(record) => {
                            record.score = expected_score;
                            WriteResult::Written
                        }
                        None => WriteResult::ConditionFailed,
                    };
                    assert_eq!(result.unwrap(), expected);
                }
                5 => {
                    let result = engine.conditional_put(
                        "profiles",
                        Bytes::from(key.clone()),
                        &[Check {
                            attribute: "count".to_owned(),
                            predicate: Predicate::GreaterThanOrEqual,
                            value: Value::Int(threshold),
                        }],
                        &[Mutation::Set(Attribute {
                            name: "name".to_owned(),
                            value: Value::String(format!("high-{raw_threshold}")),
                        })],
                    );

                    let expected = match model.get_mut(&key) {
                        Some(record) if record.count >= threshold => {
                            record.name = format!("high-{raw_threshold}");
                            WriteResult::Written
                        }
                        _ => WriteResult::ConditionFailed,
                    };
                    assert_eq!(result.unwrap(), expected);
                }
                6 => {
                    let actual = engine
                        .get("profiles", key.as_bytes())
                        .unwrap()
                        .map(|record| {
                            (
                                count_value(&record),
                                name_value(&record),
                                score_value(&record),
                            )
                        });
                    let expected = model
                        .get(&key)
                        .map(|record| (record.count, record.name.clone(), record.score));
                    assert_eq!(actual, expected);
                }
                _ => unreachable!("operation kind is bounded to 0..=6"),
            }

            assert_engine_matches_model(&engine, &model, &known_keys, threshold);
        }
    })
    .settings(Settings::new().test_cases(20))
    .run();
}
