//! Person pages (spec discover AC-05): photo, biography, dates, and every movie and
//! series as cast and per crew department, each with its library state and rating. The
//! page sorts and filters on its own (by year, by rating, „nur fehlende“, kind).

use std::collections::BTreeMap;

use axum::{
    Json,
    extract::{Path, State, rejection::PathRejection},
    http::StatusCode,
};
use jiff::Timestamp;
use serde::Serialize;
use sofarr_metadata::browse::{PersonCredit, PersonDetail, Role};

use super::{
    cache::{Cached, Freshness},
    cards::{Cards, TitleCard, image},
    failure, tmdb_client,
};
use crate::{
    api::ApiError,
    app::{App, SharedApp},
};

/// A person from the cache (24 hours) or TMDB.
pub async fn person(
    app: &App,
    person_id: i64,
    now: Timestamp,
) -> Result<Cached<PersonDetail>, ApiError> {
    let tmdb = tmdb_client(app)?;
    app.discover
        .people
        .load(&format!("person/{person_id}"), now, || {
            tmdb.person(person_id)
        })
        .await
        .map_err(|f| failure(f, "TMDB kennt diese Person nicht"))
}

/// A title of a person's credits.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct CreditCard {
    pub title: TitleCard,
    /// First release or first air date, `YYYY-MM-DD`.
    pub date: Option<String>,
    /// Cast only.
    pub character: Option<String>,
    /// Crew only, e.g. `Director` or `Writer, Story`.
    pub job: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct DepartmentCredits {
    /// TMDB's department, e.g. `Directing`, `Writing`.
    pub department: String,
    pub credits: Vec<CreditCard>,
}

/// `GET /api/discover/people/{id}`.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PersonPage {
    pub tmdb_id: i64,
    pub name: String,
    pub biography: Option<String>,
    pub birthday: Option<String>,
    pub deathday: Option<String>,
    pub place_of_birth: Option<String>,
    pub photo: Option<String>,
    /// What the person is known for, e.g. `Acting`.
    pub department: Option<String>,
    /// Newest first, undated last; empty for a person known for crew work only.
    pub cast: Vec<CreditCard>,
    /// Departments by name, credits newest first.
    pub crew: Vec<DepartmentCredits>,
    pub freshness: Freshness,
}

/// Newest first, undated last.
fn by_date(credits: &mut [CreditCard]) {
    credits.sort_by(|a, b| match (&a.date, &b.date) {
        (Some(x), Some(y)) => y.cmp(x),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.title.title.cmp(&b.title.title),
    });
}

async fn credit_card(cards: &mut Cards<'_>, credit: &PersonCredit) -> anyhow::Result<CreditCard> {
    let (character, job) = match &credit.role {
        Role::Cast { character } => (character.clone(), None),
        Role::Crew { job, .. } => (None, Some(job.clone()).filter(|j| !j.is_empty())),
    };
    Ok(CreditCard {
        title: cards.card(&credit.title).await?,
        date: credit.title.date.clone(),
        character,
        job,
    })
}

pub async fn page(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<Json<PersonPage>, ApiError> {
    let Ok(Path(person_id)) = id else {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Ungültige Person: eine TMDB-ID angeben",
        ));
    };
    let now = Timestamp::now();
    let cached = person(&app, person_id, now).await?;
    let p = cached.value.as_ref();
    let mut cards = Cards::new(&app).await?;
    let mut cast = Vec::new();
    let mut departments: BTreeMap<String, Vec<CreditCard>> = BTreeMap::new();
    for credit in &p.credits {
        let card = credit_card(&mut cards, credit).await?;
        match &credit.role {
            Role::Cast { .. } => cast.push(card),
            Role::Crew { department, .. } => {
                departments
                    .entry(department.clone())
                    .or_default()
                    .push(card);
            }
        }
    }
    cards.finish(&app);
    by_date(&mut cast);
    let crew = departments
        .into_iter()
        .map(|(department, mut credits)| {
            by_date(&mut credits);
            DepartmentCredits {
                department,
                credits,
            }
        })
        .collect();
    Ok(Json(PersonPage {
        tmdb_id: p.tmdb_id,
        name: p.name.clone(),
        biography: p.biography.clone(),
        birthday: p.birthday.clone(),
        deathday: p.deathday.clone(),
        place_of_birth: p.place_of_birth.clone(),
        photo: image(&app, p.profile_path.as_deref(), "h632"),
        department: p.department.clone(),
        cast,
        crew,
        freshness: Freshness::one(&cached, now),
    }))
}
