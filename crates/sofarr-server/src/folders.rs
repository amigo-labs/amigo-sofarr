//! Folder names of movies and series by today's naming format (spec web-ui-library
//! AC-15), and path text helpers that keep the separator style of the library paths
//! (`/` from Radarr/Sonarr, `\` for Windows folders).

use std::path::Path;

use serde_json::Value;
use sofarr_core::naming::{self, NamingConfig, NamingInput};
use sofarr_db::{Movie, Series};

use crate::{app::App, importer, series};

/// The separator a path uses: `\` only for paths written with it and without `/`.
fn separator(path: &str) -> char {
    if path.contains('\\') && !path.contains('/') {
        '\\'
    } else {
        '/'
    }
}

/// `base` + `relative` (`/`-separated) in the separator style of `base`.
pub fn join(base: &str, relative: &str) -> String {
    let sep = separator(base);
    let base = base.trim_end_matches(['/', '\\']);
    let relative: String = relative
        .trim_start_matches(['/', '\\'])
        .chars()
        .map(|c| if c == '/' || c == '\\' { sep } else { c })
        .collect();
    format!("{base}{sep}{relative}")
}

fn normalized(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_owned()
}

/// Whether two path texts name the same path (separators and a trailing one aside).
pub fn same_path(a: &str, b: &str) -> bool {
    normalized(a) == normalized(b)
}

/// The root folder an item folder lies in: the longest root folder of the setting that
/// contains it, otherwise its parent folder.
pub fn root_of(path: &str, roots: &[String]) -> String {
    let folder = normalized(path);
    roots
        .iter()
        .filter(|r| {
            let root = normalized(r);
            folder
                .strip_prefix(&root)
                .is_some_and(|rest| rest.starts_with('/') && rest.len() > 1)
        })
        .max_by_key(|r| normalized(r).len())
        .map(|r| r.trim_end_matches(['/', '\\']).to_owned())
        .unwrap_or_else(|| {
            let cut = path.trim_end_matches(['/', '\\']);
            match cut.rfind(['/', '\\']) {
                Some(0) => cut[..1].to_owned(),
                Some(i) => cut[..i].to_owned(),
                None => String::new(),
            }
        })
}

/// Paths of a `rootfolder` setting.
pub fn roots(setting: Option<&Value>) -> Vec<String> {
    setting
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|r| r["path"].as_str())
        .map(str::to_owned)
        .collect()
}

pub async fn movie_naming(app: &App) -> anyhow::Result<NamingConfig> {
    Ok(NamingConfig::from_json(
        &app.db.setting("config_naming").await?.unwrap_or_default(),
    ))
}

/// The folder a movie would get today under its root folder; `None` without a format.
pub async fn movie_folder(app: &App, movie: &Movie) -> anyhow::Result<Option<String>> {
    let naming = movie_naming(app).await?;
    let roots = roots(app.db.setting("rootfolder").await?.as_ref());
    Ok(expected_movie_folder(movie, &naming, &roots))
}

pub fn expected_movie_folder(
    movie: &Movie,
    naming: &NamingConfig,
    roots: &[String],
) -> Option<String> {
    if naming.movie_folder_format.trim().is_empty() || movie.path.is_empty() {
        return None;
    }
    let input = importer::naming_input(movie, "", "Unknown", None);
    // The same name a new movie's folder gets (`importer::movie_folder`).
    let name = importer::movie_folder(Path::new(""), &input, naming);
    let name = name.to_string_lossy();
    (!name.trim().is_empty()).then(|| join(&root_of(&movie.path, roots), &name))
}

/// The folder a series would get today under its root folder; `None` without a format.
pub async fn series_folder(app: &App, show: &Series) -> anyhow::Result<Option<String>> {
    let naming = series::naming_config(&app.db).await?;
    let roots = roots(
        app.db
            .setting(&format!("{}rootfolder", series::PREFIX))
            .await?
            .as_ref(),
    );
    Ok(expected_series_folder(show, &naming, &roots))
}

pub fn expected_series_folder(
    show: &Series,
    naming: &NamingConfig,
    roots: &[String],
) -> Option<String> {
    if naming.series_folder_format.trim().is_empty() || show.data.path.is_empty() {
        return None;
    }
    let input = NamingInput {
        title: show.data.title.clone(),
        title_de: show.data.title_de.clone(),
        year: show.data.year,
        imdb_id: show.data.imdb_id.clone(),
        ..NamingInput::default()
    };
    let name = naming::format(&naming.series_folder_format, &input, naming);
    (!name.trim().is_empty()).then(|| join(&root_of(&show.data.path, roots), &name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_keep_their_separator_style() {
        assert_eq!(
            join("/data/movies", "K/Kleiner Garten"),
            "/data/movies/K/Kleiner Garten"
        );
        assert_eq!(join("/data/movies/", "/A"), "/data/movies/A");
        assert_eq!(
            join(r"C:\lib\movies", "K/Garten"),
            r"C:\lib\movies\K\Garten"
        );
        assert!(same_path("/a/b/", r"\a\b"));
    }

    #[test]
    fn root_folders_are_found() {
        let roots = vec!["/data/media/movies".to_owned(), "/data/media".to_owned()];
        assert_eq!(
            root_of("/data/media/movies/K/Kleiner Garten (2019)", &roots),
            "/data/media/movies"
        );
        assert_eq!(root_of("/other/Film (2001)", &roots), "/other");
        assert_eq!(root_of("/Film", &[]), "/");
        assert_eq!(root_of(r"C:\lib\Film", &[]), r"C:\lib");
        assert_eq!(root_of("/data/media/movies", &roots), "/data/media");
    }
}
