//! The UI's API types (spec web-ui-shell AC-15): generated from the server's types with
//! `ts-rs` into `ui/src/lib/api/generated/`. The test fails when the committed files
//! differ; `SOFARR_UPDATE_TYPES=1` writes them instead.

use std::{collections::BTreeMap, path::PathBuf};

use ts_rs::{Config, TS};

use crate::{
    api::{ErrorBody, Health, OkBody},
    auth::{LoginBody, Me, Method},
    config::Mode,
    system::{ConnectionAnswer, ContactState, JobAnswer, JobResult, PeerKind, SystemAnswer},
};

fn folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/lib/api/generated")
}

fn one<T: TS + 'static>(cfg: &Config, out: &mut BTreeMap<String, String>) {
    let text = T::export_to_string(cfg).expect("export");
    out.insert(format!("{}.ts", T::name(cfg)), text.replace("\r\n", "\n"));
}

/// Every generated file by name.
fn generated() -> BTreeMap<String, String> {
    // Numbers stay `number`: no API value comes near 2^53.
    let cfg = Config::new().with_large_int("number");
    let mut out = BTreeMap::new();
    one::<ErrorBody>(&cfg, &mut out);
    one::<OkBody>(&cfg, &mut out);
    one::<Health>(&cfg, &mut out);
    one::<LoginBody>(&cfg, &mut out);
    one::<Me>(&cfg, &mut out);
    one::<Method>(&cfg, &mut out);
    one::<Mode>(&cfg, &mut out);
    one::<SystemAnswer>(&cfg, &mut out);
    one::<JobAnswer>(&cfg, &mut out);
    one::<JobResult>(&cfg, &mut out);
    one::<ConnectionAnswer>(&cfg, &mut out);
    one::<PeerKind>(&cfg, &mut out);
    one::<ContactState>(&cfg, &mut out);
    out
}

fn committed() -> BTreeMap<String, String> {
    std::fs::read_dir(folder())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(entry.path()).ok()?;
            name.ends_with(".ts")
                .then(|| (name, text.replace("\r\n", "\n")))
        })
        .collect()
}

#[test]
fn ui_types_match_the_server() {
    let generated = generated();
    if std::env::var_os("SOFARR_UPDATE_TYPES").is_some() {
        let dir = folder();
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("folder");
        for (name, text) in &generated {
            std::fs::write(dir.join(name), text).expect("write");
        }
        return;
    }
    let committed = committed();
    let differing: Vec<&String> = generated
        .keys()
        .chain(committed.keys())
        .filter(|name| generated.get(*name) != committed.get(*name))
        .collect();
    assert!(
        differing.is_empty(),
        "UI types differ from the server's in {differing:?}; \
         regenerate with SOFARR_UPDATE_TYPES=1 cargo test -p sofarr-server ui_types"
    );
}
