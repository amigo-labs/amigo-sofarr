//! `compare-rename --app sonarr` (spec web-ui-library AC-23): sofarr's episode file
//! names (the rename preview, i.e. the import's naming) against Sonarr's rename preview
//! (`/api/v3/rename?seriesId=…`, which lists only the files Sonarr would rename).

use std::{collections::BTreeMap, fmt::Write as _};

use serde_json::Value;

use crate::{app::App, library_files::series_renames, radarr::Radarr};

fn slashes(path: &str) -> String {
    path.replace('\\', "/")
}

/// Compares the file names of up to `limit` series known to Sonarr.
pub async fn series(app: &App, sonarr: &Radarr, limit: usize) -> anyhow::Result<(String, bool)> {
    let shows: Vec<_> = app
        .db
        .all_series()
        .await?
        .into_iter()
        .filter(|s| s.data.sonarr_id.is_some())
        .take(limit)
        .collect();
    let mut compared = 0;
    let mut mismatches = Vec::new();
    for show in &shows {
        let Some(sonarr_id) = show.data.sonarr_id else {
            continue;
        };
        let id = sonarr_id.to_string();
        let theirs: BTreeMap<String, String> = sonarr
            .get("rename", &[("seriesId", id.as_str())])
            .await?
            .as_array()
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .filter_map(|r: &Value| {
                Some((
                    slashes(r["existingPath"].as_str()?),
                    slashes(r["newPath"].as_str()?),
                ))
            })
            .collect();
        let ours: BTreeMap<String, String> = series_renames(app, show)
            .await?
            .into_iter()
            .map(|r| (r.old_path, r.new_path))
            .collect();
        let mut files: Vec<String> = app
            .db
            .episodes(Some(show.id))
            .await?
            .into_iter()
            .filter_map(|e| e.file.map(|f| f.relative_path))
            .collect();
        files.sort();
        files.dedup();
        for file in files {
            compared += 1;
            let their_name = theirs.get(&file).unwrap_or(&file);
            let our_name = ours.get(&file).unwrap_or(&file);
            if their_name != our_name {
                mismatches.push(format!(
                    "  {}: {file}\n    Sonarr: {their_name}\n    sofarr: {our_name}",
                    show.data.title
                ));
            }
        }
    }
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Dateinamen (Serien): {compared} verglichen, {} abweichend",
        mismatches.len()
    );
    for m in mismatches.iter().take(30) {
        let _ = writeln!(out, "{m}");
    }
    Ok((out, mismatches.is_empty()))
}

#[cfg(test)]
mod tests {
    use axum::{Json, Router, extract::Query, routing::get};
    use rand::RngCore;
    use serde_json::json;

    use super::*;
    use crate::testkit::{Library, app};

    /// A fake Sonarr whose rename preview holds `renames` for series 3.
    async fn sonarr(renames: Value) -> Radarr {
        let router = Router::new().route(
            "/api/v3/rename",
            get(move |Query(q): Query<BTreeMap<String, String>>| {
                let renames = renames.clone();
                async move {
                    Json(if q.get("seriesId").map(String::as_str) == Some("3") {
                        renames
                    } else {
                        json!([])
                    })
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move { axum::serve(listener, router).await });
        let mut key = [0u8; 8];
        rand::rngs::OsRng.fill_bytes(&mut key);
        Radarr::new(
            reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("client"),
            &crate::config::RadarrConfig {
                url: format!("http://{addr}"),
                api_key: key.iter().map(|b| format!("{b:02x}")).collect(),
            },
        )
    }

    const OURS: &str = "Staffel 01/Kleine Insel - S01E01 - Ankunft WEBDL-1080p.mkv";

    #[tokio::test]
    async fn reports_names_per_file_against_sonarr() {
        let app = app(Library::Series, "").await;
        let same = sonarr(json!([{
            "seriesId": 3, "seasonNumber": 1, "episodeNumbers": [1], "episodeFileId": 300,
            "existingPath": "Staffel 01\\Kleine Insel - S01E01.mkv", "newPath": OURS
        }]))
        .await;
        let (report, clean) = series(&app, &same, 10).await.expect("compare");
        assert!(clean, "{report}");
        assert!(report.starts_with("Dateinamen (Serien): 1 verglichen, 0 abweichend"));

        // Sonarr keeps the name (not in its list): sofarr would rename, that differs.
        let keeps = sonarr(json!([])).await;
        let (report, clean) = series(&app, &keeps, 10).await.expect("compare");
        assert!(!clean);
        assert!(report.contains("1 abweichend"), "{report}");
        assert!(
            report.contains("Sonarr: Staffel 01/Kleine Insel - S01E01.mkv"),
            "{report}"
        );
        assert!(report.contains(&format!("sofarr: {OURS}")), "{report}");
    }
}
