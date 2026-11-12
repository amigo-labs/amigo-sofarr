//! `amigo-sofarr search` exits with an error for unknown or malformed targets
//! (spec fail-handling AC-13).

use std::process::{Command, Output};

fn run(config: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_amigo-sofarr"))
        .arg("--config")
        .arg(config)
        .args(args)
        .output()
        .expect("binary runs")
}

#[test]
fn search_rejects_unknown_and_malformed_targets() {
    let dir = std::env::temp_dir().join(format!("sofarr-cli-search-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let config = dir.join("sofarr.toml");
    std::fs::write(
        &config,
        format!(
            "[database]\npath = '{}'\n[log]\nlevel = \"error\"\n",
            dir.join("sofarr.db").display()
        ),
    )
    .expect("config");

    let unknown_movie = run(&config, &["search", "999"]);
    assert!(!unknown_movie.status.success());
    let stderr = String::from_utf8_lossy(&unknown_movie.stderr);
    assert!(
        stderr.contains("Film 999 ist nicht in der Bibliothek"),
        "{stderr}"
    );

    let unknown_series = run(&config, &["search", "--series", "5", "--season", "1"]);
    assert!(!unknown_series.status.success());
    let stderr = String::from_utf8_lossy(&unknown_series.stderr);
    assert!(
        stderr.contains("Serie 5 ist nicht in der Bibliothek"),
        "{stderr}"
    );

    let missing = run(&config, &["search"]);
    assert!(!missing.status.success());
    let stderr = String::from_utf8_lossy(&missing.stderr);
    assert!(stderr.contains("Suchziel fehlt"), "{stderr}");

    for args in [
        &["search", "--season", "1"][..],
        &["search", "--series", "5", "--episode", "2"],
        &["search", "7", "--series", "5"],
    ] {
        assert!(!run(&config, args).status.success(), "{args:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
