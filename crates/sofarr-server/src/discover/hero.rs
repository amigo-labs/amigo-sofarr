//! The hero of the discover start page (spec ui-cinema-hero AC-01, AC-06, AC-07): the
//! first title of „Heute beliebt“ that is neither in the library nor excluded and whose
//! preview has a backdrop.

use std::sync::Arc;

use jiff::Timestamp;
use serde::Serialize;
use sofarr_metadata::browse::{Kind, TitleDetail, TitleSummary};
use tokio::task::JoinSet;

use super::{
    Failure,
    cache::Cached,
    cards::{Cards, LibraryState, LibraryStatus, Rating, TitleKind, image},
    titles::title_key,
};
use crate::app::{App, SharedApp};

/// Candidates looked at, at most (AC-01).
pub const CANDIDATES: usize = 10;

/// The featured title (AC-06).
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Hero {
    pub kind: TitleKind,
    pub tmdb_id: i64,
    /// German title where TMDB has one, else the original one.
    pub title: String,
    pub year: Option<u16>,
    pub overview: Option<String>,
    /// sofarr's image URL of the backdrop (D24).
    pub backdrop: String,
    pub rating: Rating,
    pub library: LibraryState,
}

/// The featured title among `titles` (the shelf „Heute beliebt“, in its order) and its
/// cached preview, whose time the page's freshness names (AC-08).
pub async fn choose(
    app: &SharedApp,
    cards: &mut Cards<'_>,
    titles: &[TitleSummary],
    now: Timestamp,
) -> anyhow::Result<Option<(Hero, Cached<TitleDetail>)>> {
    let candidates: Vec<(Kind, i64)> = titles
        .iter()
        .map(|t| (t.kind, t.tmdb_id))
        .filter(|(kind, id)| {
            // A series is found in the library by its TVDB id, which only its preview
            // names; without a cached one it counts as a candidate until it is loaded.
            let cached = app.discover.titles.any(&title_key(*kind, *id));
            let detail = cached.as_ref().map(|c| c.value.as_ref());
            cards.library.state(*kind, *id, detail).0.status == LibraryStatus::None
        })
        .take(CANDIDATES)
        .collect();
    let mut previews = preview_all(app, &candidates, now).await;
    for (n, (kind, id)) in candidates.into_iter().enumerate() {
        let Some(preview) = previews.get_mut(n).and_then(Option::take) else {
            continue;
        };
        let d = preview.value.as_ref();
        if cards.library.state(kind, id, Some(d)).0.status != LibraryStatus::None {
            continue;
        }
        let Some(backdrop) = image(app, d.backdrop_path.as_deref(), "w1280") else {
            continue;
        };
        let (library, rating) = cards.state(kind, id).await?;
        let hero = Hero {
            kind: TitleKind::of(kind),
            tmdb_id: id,
            title: d.summary.title.clone(),
            year: d.summary.year,
            overview: d.overview.clone().filter(|o| !o.trim().is_empty()),
            backdrop,
            rating,
            library,
        };
        return Ok(Some((hero, preview)));
    }
    Ok(None)
}

/// The previews of the candidates, from the cache or asked of TMDB all at once (AC-07);
/// `None` where TMDB has none or cannot be reached and nothing is cached.
async fn preview_all(
    app: &SharedApp,
    candidates: &[(Kind, i64)],
    now: Timestamp,
) -> Vec<Option<Cached<TitleDetail>>> {
    let mut tasks = JoinSet::new();
    for (n, (kind, id)) in candidates.iter().copied().enumerate() {
        let app = Arc::clone(app);
        tasks.spawn(async move { (n, preview(&app, kind, id, now).await) });
    }
    let mut out = vec![None; candidates.len()];
    while let Some(done) = tasks.join_next().await {
        if let Ok((n, Some(preview))) = done
            && let Some(slot) = out.get_mut(n)
        {
            *slot = Some(preview);
        }
    }
    out
}

async fn preview(app: &App, kind: Kind, id: i64, now: Timestamp) -> Option<Cached<TitleDetail>> {
    let key = title_key(kind, id);
    // TMDB said it knows no such title: no preview to wait for.
    if app.discover.titles.any(&key).is_none()
        && app.discover.unknown.lock().is_ok_and(|u| u.contains(&key))
    {
        return None;
    }
    let tmdb = crate::metadata::tmdb(app).ok()?;
    match app
        .discover
        .titles
        .load(&key, now, || tmdb.title(kind, id))
        .await
    {
        Ok(preview) => Some(preview),
        Err(failure) => {
            if failure == Failure::NotFound
                && let Ok(mut unknown) = app.discover.unknown.lock()
            {
                unknown.insert(key);
            }
            None
        }
    }
}
