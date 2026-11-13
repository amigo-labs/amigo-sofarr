//! `amigo-sofarr hash-password` (spec auth AC-13).

use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

use argon2::{Argon2, PasswordHash, PasswordVerifier};

fn hash_password(input: &[u8]) -> Output {
    let dir = std::env::temp_dir().join(format!("sofarr-cli-auth-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let config = dir.join("sofarr.toml");
    std::fs::write(&config, "[log]\nlevel = \"error\"\n").expect("config");
    let mut child = Command::new(env!("CARGO_BIN_EXE_amigo-sofarr"))
        .arg("--config")
        .arg(&config)
        .arg("hash-password")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input)
        .expect("write");
    child.wait_with_output().expect("output")
}

#[test]
fn hashes_a_piped_password() {
    let output = hash_password(b"Sofa-Passwort-91\n");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let hash = stdout.trim();
    assert!(hash.starts_with("$argon2id$"), "{hash}");
    assert!(!stdout.contains("Sofa-Passwort-91"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Sofa-Passwort-91"));
    let parsed = PasswordHash::new(hash).expect("PHC string");
    assert!(
        Argon2::default()
            .verify_password(b"Sofa-Passwort-91", &parsed)
            .is_ok()
    );
}

#[test]
fn works_while_the_auth_config_is_still_incomplete() {
    let dir = std::env::temp_dir().join(format!("sofarr-cli-auth-half-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let config = dir.join("sofarr.toml");
    // Auth switched on, but the hash still missing: the very moment to create one.
    std::fs::write(&config, "[auth]\nenabled = true\nusername = \"daniel\"\n").expect("config");
    let mut child = Command::new(env!("CARGO_BIN_EXE_amigo-sofarr"))
        .arg("--config")
        .arg(&config)
        .arg("hash-password")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"Sofa-Passwort-91\n")
        .expect("write");
    let output = child.wait_with_output().expect("output");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("$argon2id$"));
}

#[test]
fn refuses_an_empty_password() {
    let output = hash_password(b"\n");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Leeres Passwort"), "{stderr}");
    assert!(output.stdout.is_empty());
}
