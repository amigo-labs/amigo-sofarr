//! `amigo-sofarr sync-import-lists` runs the import list sync once and prints how many
//! movies it added (spec import-lists).

use std::process::Command;

#[test]
fn sync_import_lists_prints_the_added_count() {
    let dir = std::env::temp_dir().join(format!("sofarr-cli-lists-{}", std::process::id()));
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

    let output = Command::new(env!("CARGO_BIN_EXE_amigo-sofarr"))
        .arg("--config")
        .arg(&config)
        .arg("sync-import-lists")
        .output()
        .expect("binary runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(stdout.trim(), "Hinzugefügt: 0");
    let _ = std::fs::remove_dir_all(&dir);
}
