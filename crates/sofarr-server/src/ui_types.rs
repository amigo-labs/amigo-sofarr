//! The UI's API types (spec web-ui-shell AC-15): generated from the server's types with
//! `ts-rs` into `ui/src/lib/api/generated/`. The test fails when the committed files
//! differ; `SOFARR_UPDATE_TYPES=1` writes them instead.

use std::{collections::BTreeMap, path::PathBuf};

use ts_rs::{Config, TS};

use crate::{
    api::{BlocklistEntry, BlocklistPage, PageQuery, RestoreAnswer, SearchBody},
    api::{ErrorBody, Health, OkBody},
    auth::{LoginBody, Me, Method},
    backup_files::{BackupFile, Kind},
    backups::{BackupError, BackupList},
    calendar::{CalendarAnswer, CalendarEntry, CalendarQuery, EntryKind, EntryState, Release},
    config::Mode,
    library_detail::{
        BlockedRelease, EpisodeAnswer, EpisodeDetail, FileActions, FileDetail, FolderState,
        GrabEntry, MovieDetail, ReleaseDates, SeasonAnswer, SeriesDetail,
    },
    library_edit::{
        ChangePreview, ItemKind, MonitorAnswer, MonitorBody, MonitorItem, MovieChange, PreviewItem,
        SeriesChange,
    },
    library_files::{
        FolderAnswer, FolderPreview, RemoveAnswer, RemoveBody, RenameAnswer, RenameEntry,
        RenamePreview,
    },
    library_list::Page,
    library_list::{
        Choice, LibraryOptions, MovieListItem, MovieListQuery, Order, SeriesListItem,
        SeriesListQuery, Sort, Tri,
    },
    library_view::ReleaseStage,
    overview::{ActivityEntry, ActivityQuery, MediaKind, OverviewAnswer, QueueAnswer, QueueItem},
    search::SearchReport,
    system::{ConnectionAnswer, ContactState, JobAnswer, JobResult, PeerKind, SystemAnswer},
    wanted::{
        BulkSearchBody, BulkSearchReport, SearchItem, WantedItem, WantedList, WantedMedia,
        WantedQuery, WantedSelector,
    },
};

fn folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/lib/api/generated")
}

fn one<T: TS + 'static>(cfg: &Config, out: &mut BTreeMap<String, String>) {
    let text = T::export_to_string(cfg).expect("export");
    // `ident`: a generic type's file is named without its parameters.
    out.insert(format!("{}.ts", T::ident(cfg)), text.replace("\r\n", "\n"));
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
    one::<SearchBody>(&cfg, &mut out);
    one::<PageQuery>(&cfg, &mut out);
    one::<BlocklistPage>(&cfg, &mut out);
    one::<BlocklistEntry>(&cfg, &mut out);
    one::<RestoreAnswer>(&cfg, &mut out);
    one::<BackupList>(&cfg, &mut out);
    one::<BackupError>(&cfg, &mut out);
    one::<BackupFile>(&cfg, &mut out);
    one::<Kind>(&cfg, &mut out);
    one::<SearchReport>(&cfg, &mut out);
    one::<Tri>(&cfg, &mut out);
    one::<Sort>(&cfg, &mut out);
    one::<Order>(&cfg, &mut out);
    one::<MovieListQuery>(&cfg, &mut out);
    one::<SeriesListQuery>(&cfg, &mut out);
    one::<MovieListItem>(&cfg, &mut out);
    one::<SeriesListItem>(&cfg, &mut out);
    one::<Choice>(&cfg, &mut out);
    one::<LibraryOptions>(&cfg, &mut out);
    one::<ReleaseStage>(&cfg, &mut out);
    one::<FileActions>(&cfg, &mut out);
    one::<FolderState>(&cfg, &mut out);
    one::<FileDetail>(&cfg, &mut out);
    one::<GrabEntry>(&cfg, &mut out);
    one::<BlockedRelease>(&cfg, &mut out);
    one::<ReleaseDates>(&cfg, &mut out);
    one::<MovieDetail>(&cfg, &mut out);
    one::<EpisodeAnswer>(&cfg, &mut out);
    one::<SeasonAnswer>(&cfg, &mut out);
    one::<SeriesDetail>(&cfg, &mut out);
    one::<EpisodeDetail>(&cfg, &mut out);
    one::<MonitorItem>(&cfg, &mut out);
    one::<MonitorBody>(&cfg, &mut out);
    one::<MonitorAnswer>(&cfg, &mut out);
    one::<MovieChange>(&cfg, &mut out);
    one::<SeriesChange>(&cfg, &mut out);
    one::<ItemKind>(&cfg, &mut out);
    one::<PreviewItem>(&cfg, &mut out);
    one::<ChangePreview>(&cfg, &mut out);
    one::<WantedList>(&cfg, &mut out);
    one::<WantedMedia>(&cfg, &mut out);
    one::<WantedQuery>(&cfg, &mut out);
    one::<WantedItem>(&cfg, &mut out);
    one::<SearchItem>(&cfg, &mut out);
    one::<WantedSelector>(&cfg, &mut out);
    one::<BulkSearchBody>(&cfg, &mut out);
    one::<BulkSearchReport>(&cfg, &mut out);
    one::<MediaKind>(&cfg, &mut out);
    one::<ActivityEntry>(&cfg, &mut out);
    one::<QueueItem>(&cfg, &mut out);
    one::<QueueAnswer>(&cfg, &mut out);
    one::<OverviewAnswer>(&cfg, &mut out);
    one::<ActivityQuery>(&cfg, &mut out);
    one::<EntryState>(&cfg, &mut out);
    one::<Release>(&cfg, &mut out);
    one::<EntryKind>(&cfg, &mut out);
    one::<CalendarEntry>(&cfg, &mut out);
    one::<CalendarAnswer>(&cfg, &mut out);
    one::<CalendarQuery>(&cfg, &mut out);
    one::<RenameEntry>(&cfg, &mut out);
    one::<RenamePreview>(&cfg, &mut out);
    one::<RenameAnswer>(&cfg, &mut out);
    one::<FolderPreview>(&cfg, &mut out);
    one::<FolderAnswer>(&cfg, &mut out);
    one::<RemoveBody>(&cfg, &mut out);
    one::<RemoveAnswer>(&cfg, &mut out);
    one::<Page<()>>(&cfg, &mut out);
    one::<crate::imdb::ImdbInfo>(&cfg, &mut out);
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
