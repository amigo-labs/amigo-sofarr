//! Import lists: TMDb person lists (spec D11). New movies of a person are added to the
//! library with the list's profile, root folder and monitoring.

use std::collections::BTreeSet;

use serde_json::Value;
use sofarr_metadata::CreditedMovie;

/// One `TMDbPersonImport` list from the `importlist` settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonList {
    pub name: String,
    pub person_id: i64,
    pub cast: bool,
    /// Crew departments to include (`Directing`, `Production`, `Sound`, `Writing`).
    pub departments: Vec<&'static str>,
    pub monitored: bool,
    pub quality_profile_id: i64,
    pub root_folder: String,
    pub search_on_add: bool,
    /// `minimumAvailability` of added movies.
    pub minimum_availability: String,
    /// Tag ids the added movies get.
    pub tags: Vec<i64>,
}

fn field<'a>(list: &'a Value, name: &str) -> Option<&'a Value> {
    list["fields"]
        .as_array()?
        .iter()
        .find(|f| f["name"].as_str() == Some(name))
        .map(|f| &f["value"])
}

/// Reads the enabled, automatic person lists; returns warnings for unusable entries.
pub fn person_lists(settings: &Value) -> (Vec<PersonList>, Vec<String>) {
    let mut lists = Vec::new();
    let mut warnings = Vec::new();
    for list in settings.as_array().map_or(&[][..], Vec::as_slice) {
        let name = list["name"].as_str().unwrap_or("?").to_owned();
        if list["implementation"].as_str() != Some("TMDbPersonImport") {
            continue;
        }
        if list["enabled"].as_bool() == Some(false) || list["enableAuto"].as_bool() == Some(false) {
            continue;
        }
        let person_id = field(list, "personId").and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
        });
        let Some(person_id) = person_id else {
            warnings.push(format!("Import List {name}: personId fehlt"));
            continue;
        };
        let flag = |n: &str| field(list, n).and_then(Value::as_bool).unwrap_or(false);
        let departments = [
            ("personCastDirector", "Directing"),
            ("personCastProducer", "Production"),
            ("personCastSound", "Sound"),
            ("personCastWriting", "Writing"),
        ]
        .into_iter()
        .filter(|(f, _)| flag(f))
        .map(|(_, d)| d)
        .collect();
        let Some(quality_profile_id) = list["qualityProfileId"].as_i64() else {
            warnings.push(format!("Import List {name}: qualityProfileId fehlt"));
            continue;
        };
        lists.push(PersonList {
            name,
            person_id,
            cast: field(list, "personCast")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            departments,
            monitored: list["monitor"].as_str().is_none_or(|m| m != "none"),
            quality_profile_id,
            root_folder: list["rootFolderPath"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            search_on_add: list["searchOnAdd"].as_bool().unwrap_or(false),
            minimum_availability: list["minimumAvailability"]
                .as_str()
                .unwrap_or("released")
                .to_owned(),
            tags: crate::library::tag_ids(&list["tags"]),
        });
    }
    (lists, warnings)
}

/// TMDB ids excluded from import lists (`exclusions` settings).
pub fn exclusions(settings: Option<&Value>) -> BTreeSet<i64> {
    settings
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|e| e["tmdbId"].as_i64())
        .collect()
}

/// The movies of a person that the list selects.
pub fn select<'a>(list: &PersonList, credits: &'a [CreditedMovie]) -> Vec<&'a CreditedMovie> {
    credits
        .iter()
        .filter(|c| (list.cast && c.role == "cast") || list.departments.contains(&c.role.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_person_lists() {
        let settings = json!([
            {"name": "Regisseurin", "implementation": "TMDbPersonImport", "enabled": true, "enableAuto": true,
             "monitor": "movieOnly", "qualityProfileId": 4, "rootFolderPath": "/data/media/movies",
             "searchOnAdd": true, "minimumAvailability": "inCinemas", "tags": [2],
             "fields": [{"name": "personId", "value": "1234"}, {"name": "personCast", "value": false},
                        {"name": "personCastDirector", "value": true}]},
            {"name": "Aus", "implementation": "TMDbPersonImport", "enabled": false, "fields": []},
            {"name": "Kaputt", "implementation": "TMDbPersonImport", "qualityProfileId": 1, "fields": []},
            {"name": "Andere", "implementation": "TraktListImport"}
        ]);
        let (lists, warnings) = person_lists(&settings);
        assert_eq!(lists.len(), 1);
        assert_eq!(warnings, ["Import List Kaputt: personId fehlt"]);
        let list = &lists[0];
        assert_eq!(list.person_id, 1234);
        assert!(!list.cast);
        assert_eq!(list.departments, ["Directing"]);
        assert!(list.monitored && list.search_on_add);
        assert_eq!(list.minimum_availability, "inCinemas");
        assert_eq!(list.tags, [2]);

        let credits = vec![
            CreditedMovie {
                tmdb_id: 1,
                title: "A".to_owned(),
                year: None,
                role: "cast".to_owned(),
            },
            CreditedMovie {
                tmdb_id: 2,
                title: "B".to_owned(),
                year: None,
                role: "Directing".to_owned(),
            },
        ];
        assert_eq!(
            select(list, &credits)
                .iter()
                .map(|c| c.tmdb_id)
                .collect::<Vec<_>>(),
            [2]
        );
        assert_eq!(
            exclusions(Some(&json!([{"tmdbId": 5, "movieTitle": "x"}]))),
            BTreeSet::from([5])
        );
    }
}
