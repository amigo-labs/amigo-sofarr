//! `amigo-sofarr compare-mediainfo` stops with a German message and an exit code other
//! than 0 when the oracle cannot be reached or refuses the API key (spec mediainfo AC-09).

use std::{
    io::{Read, Write},
    net::TcpListener,
    process::{Command, Output},
};

fn run(config: &std::path::Path, app: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_amigo-sofarr"))
        .arg("--config")
        .arg(config)
        .args(["compare-mediainfo", "--app", app])
        .output()
        .expect("binary runs")
}

/// An oracle that answers every request with 401.
fn refusing_oracle() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", listener.local_addr().expect("addr"));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten().take(16) {
            let mut stream = stream;
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(
                b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        }
    });
    url
}

/// A URL nothing listens on.
fn closed_port() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", listener.local_addr().expect("addr"));
    drop(listener);
    url
}

fn config(dir: &std::path::Path, radarr: &str, sonarr: &str) -> std::path::PathBuf {
    let config = dir.join("sofarr.toml");
    std::fs::write(
        &config,
        format!(
            "[database]\npath = '{}'\n[backup]\ndir = '{}'\n[log]\nlevel = \"error\"\n\
             [radarr]\nurl = \"{radarr}\"\napi_key = \"k\"\n[sonarr]\nurl = \"{sonarr}\"\napi_key = \"k\"\n",
            dir.join("sofarr.db").display(),
            dir.join("backups").display(),
        ),
    )
    .expect("config");
    config
}

#[test]
fn an_unreachable_or_refusing_oracle_ends_with_a_german_error() {
    let dir = std::env::temp_dir().join(format!("sofarr-cli-mediainfo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");

    let unreachable = config(&dir, &closed_port(), &closed_port());
    for (app, name) in [("radarr", "Radarr"), ("sonarr", "Sonarr")] {
        let out = run(&unreachable, app);
        assert!(!out.status.success(), "{app}: exit code 0");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("nicht abrufbar"), "{app}: {stderr}");
        assert!(stderr.contains(name), "{app}: {stderr}");
    }

    let refusing = config(&dir, &refusing_oracle(), &refusing_oracle());
    for app in ["radarr", "sonarr"] {
        let out = run(&refusing, app);
        assert!(!out.status.success(), "{app}: exit code 0");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("API-Key abgelehnt"), "{app}: {stderr}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
