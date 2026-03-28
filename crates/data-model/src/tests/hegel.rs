use std::sync::{Mutex, OnceLock};

use ::hegel::{Hegel, Settings, TestCase, generators};

use super::*;

static HEGEL_SERVER_COMMAND: OnceLock<String> = OnceLock::new();
static HEGEL_ENV_LOCK: Mutex<()> = Mutex::new(());

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

fn generated_space(
    space_id: u8,
    attr_specs: &[(u8, u8, u8, u8)],
    subspace_specs: &[(u8, u8, u8)],
    fault_tolerance: u8,
    partitions: u8,
) -> Space {
    let attributes = attr_specs
        .iter()
        .enumerate()
        .map(|(idx, &(a, b, c, d))| AttributeDefinition {
            name: format!("attr{idx}"),
            kind: generated_value_kind([a, b, c, d], 2),
        })
        .collect::<Vec<_>>();
    let subspaces = subspace_specs
        .iter()
        .map(|&(count_raw, first_raw, second_raw)| Subspace {
            dimensions: generated_dimensions(attributes.len(), count_raw, first_raw, second_raw),
        })
        .collect::<Vec<_>>();

    Space {
        name: format!("space_{space_id}"),
        key_attribute: format!("key_{space_id}"),
        attributes,
        subspaces,
        options: SpaceOptions {
            fault_tolerance: u32::from(fault_tolerance % 4),
            partitions: u32::from(partitions) + 1,
            schema_format: SchemaFormat::HyperDexDsl,
        },
    }
}

fn generated_dimensions(
    attribute_count: usize,
    count_raw: u8,
    first_raw: u8,
    second_raw: u8,
) -> Vec<String> {
    let mut dimensions = Vec::new();
    let desired = usize::from(count_raw % 3) + 1;

    for raw in [first_raw, second_raw, first_raw.wrapping_add(second_raw)] {
        let name = format!("attr{}", usize::from(raw) % attribute_count);
        if !dimensions.contains(&name) {
            dimensions.push(name);
        }
        if dimensions.len() == desired {
            break;
        }
    }

    if dimensions.is_empty() {
        dimensions.push("attr0".to_owned());
    }

    dimensions
}

fn generated_value_kind(codes: [u8; 4], depth: usize) -> ValueKind {
    let mut cursor = 0usize;
    generated_value_kind_inner(&codes, &mut cursor, depth)
}

fn generated_value_kind_inner(codes: &[u8; 4], cursor: &mut usize, depth: usize) -> ValueKind {
    let code = codes[*cursor % codes.len()];
    *cursor += 1;

    if depth == 0 {
        return generated_leaf_kind(code);
    }

    match code % 10 {
        0 => ValueKind::Bool,
        1 => ValueKind::Int,
        2 => ValueKind::Float,
        3 => ValueKind::Bytes,
        4 => ValueKind::String,
        5 => ValueKind::Document,
        6 => ValueKind::Timestamp(generated_time_unit(code)),
        7 => ValueKind::List(Box::new(generated_value_kind_inner(
            codes,
            cursor,
            depth - 1,
        ))),
        8 => ValueKind::Set(Box::new(generated_value_kind_inner(
            codes,
            cursor,
            depth - 1,
        ))),
        _ => ValueKind::Map {
            key: Box::new(generated_value_kind_inner(codes, cursor, depth - 1)),
            value: Box::new(generated_value_kind_inner(codes, cursor, depth - 1)),
        },
    }
}

fn generated_leaf_kind(code: u8) -> ValueKind {
    match code % 7 {
        0 => ValueKind::Bool,
        1 => ValueKind::Int,
        2 => ValueKind::Float,
        3 => ValueKind::Bytes,
        4 => ValueKind::String,
        5 => ValueKind::Document,
        _ => ValueKind::Timestamp(generated_time_unit(code)),
    }
}

fn generated_time_unit(code: u8) -> TimeUnit {
    match code % 6 {
        0 => TimeUnit::Second,
        1 => TimeUnit::Minute,
        2 => TimeUnit::Hour,
        3 => TimeUnit::Day,
        4 => TimeUnit::Week,
        _ => TimeUnit::Month,
    }
}

#[test]
fn hegel_hyperdex_schema_render_parse_round_trips_supported_spaces() {
    let _guard = HEGEL_ENV_LOCK.lock().unwrap();
    let hegel_server_command = ensure_hegel_server_command();
    unsafe {
        std::env::set_var("HEGEL_SERVER_COMMAND", &hegel_server_command);
    }

    Hegel::new(|tc: TestCase| {
        let space_id: u8 = tc.draw(generators::integers::<u8>().max_value(20));
        let attr_specs: Vec<(u8, u8, u8, u8)> = tc.draw(
            generators::vecs(generators::tuples4(
                generators::integers::<u8>(),
                generators::integers::<u8>(),
                generators::integers::<u8>(),
                generators::integers::<u8>(),
            ))
            .min_size(1)
            .max_size(6),
        );
        let subspace_specs: Vec<(u8, u8, u8)> = tc.draw(
            generators::vecs(generators::tuples3(
                generators::integers::<u8>(),
                generators::integers::<u8>(),
                generators::integers::<u8>(),
            ))
            .max_size(4),
        );
        let fault_tolerance: u8 = tc.draw(generators::integers::<u8>().max_value(7));
        let partitions: u8 = tc.draw(generators::integers::<u8>().max_value(63));

        let schema = generated_space(
            space_id,
            &attr_specs,
            &subspace_specs,
            fault_tolerance,
            partitions,
        );
        let rendered = format_hyperdex_space(&schema);
        let reparsed = parse_hyperdex_space(&rendered).unwrap();

        assert_eq!(reparsed, schema, "rendered schema:\n{rendered}");
    })
    .settings(Settings::new().test_cases(40))
    .run();
}
