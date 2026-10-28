//! The exported Radarr config (`sofarr-parity export-public`) imports cleanly.
//! Skips while `fixtures/public/config/radarr/` has not been committed yet.

use std::{fs, path::Path};

use sofarr_core::import;

#[test]
fn public_config_imports() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/public/config/radarr");
    let read = |name: &str| fs::read_to_string(dir.join(name)).ok();
    let (Some(definitions), Some(profiles), Some(formats)) = (
        read("qualitydefinition.json"),
        read("qualityprofile.json"),
        read("customformat.json"),
    ) else {
        eprintln!(
            "übersprungen: keine öffentliche Config unter {}",
            dir.display()
        );
        return;
    };
    let settings = read("settings.json");

    let config = import::radarr(&definitions, &profiles, &formats, settings.as_deref())
        .unwrap_or_else(|e| panic!("Import fehlgeschlagen: {e}"));

    let catalog = config.definitions.catalog();
    for profile in &config.profiles {
        assert!(
            profile.cutoff_rank().is_some(),
            "Profil {}: Cutoff {} nicht in den Items",
            profile.name,
            profile.cutoff
        );
        for item in &profile.items {
            let ids: Vec<_> = match item {
                sofarr_core::ProfileItem::Quality { quality, .. } => vec![*quality],
                sofarr_core::ProfileItem::Group { qualities, .. } => qualities.clone(),
            };
            for id in ids {
                assert!(
                    catalog.by_id(id).is_some(),
                    "Profil {}: Quality {id} fehlt in den Definitions",
                    profile.name
                );
            }
        }
        for (format, _) in &profile.format_scores {
            assert!(
                config.custom_formats.iter().any(|cf| cf.id == *format),
                "Profil {}: Custom Format {format} unbekannt",
                profile.name
            );
        }
    }
}
