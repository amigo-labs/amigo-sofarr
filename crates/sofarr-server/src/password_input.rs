//! `hash-password` (spec auth AC-13): the password comes from the terminal without echo,
//! or as one line on stdin when it is piped.

use std::io::BufRead;

use anyhow::Context;

use crate::auth;

/// Reads the password — from `prompt` when stdin is a terminal, otherwise one line of
/// `piped` — and returns its Argon2id hash. An empty password is refused.
pub fn hash_from_input(
    terminal: bool,
    prompt: impl FnOnce() -> std::io::Result<String>,
    piped: &mut dyn BufRead,
) -> anyhow::Result<String> {
    let line = if terminal {
        prompt().context("Passwort nicht lesbar")?
    } else {
        let mut line = String::new();
        piped
            .read_line(&mut line)
            .context("Passwort nicht lesbar")?;
        line
    };
    let password = line.trim_end_matches(['\r', '\n']);
    anyhow::ensure!(!password.is_empty(), "Leeres Passwort – kein Hash erzeugt");
    auth::hash_password(password)
}

/// The CLI command: the terminal prompt hides what is typed.
pub fn run() -> anyhow::Result<()> {
    use std::io::IsTerminal;
    let stdin = std::io::stdin();
    let hash = hash_from_input(
        stdin.is_terminal(),
        || rpassword::prompt_password("Passwort: "),
        &mut stdin.lock(),
    )?;
    println!("{hash}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, io::Cursor};

    use super::*;

    fn verifies(password: &str, hash: &str) -> bool {
        use argon2::{PasswordHash, PasswordVerifier};
        PasswordHash::new(hash).is_ok_and(|h| {
            argon2::Argon2::default()
                .verify_password(password.as_bytes(), &h)
                .is_ok()
        })
    }

    #[test]
    fn a_typed_password_comes_from_the_hidden_prompt() {
        let prompted = Cell::new(false);
        let hash = hash_from_input(
            true,
            || {
                prompted.set(true);
                Ok("im Terminal getippt".to_owned())
            },
            &mut Cursor::new("nicht gelesen\n"),
        )
        .expect("hash");
        assert!(prompted.get());
        assert!(verifies("im Terminal getippt", &hash));
        assert!(!verifies("nicht gelesen", &hash));
    }

    #[test]
    fn a_piped_password_is_one_line_of_stdin() {
        let prompted = Cell::new(false);
        let hash = hash_from_input(
            false,
            || {
                prompted.set(true);
                Ok(String::new())
            },
            &mut Cursor::new("gepipet\r\nzweite Zeile\n"),
        )
        .expect("hash");
        assert!(!prompted.get());
        assert!(verifies("gepipet", &hash));
    }

    #[test]
    fn an_empty_or_unreadable_password_makes_no_hash() {
        let empty =
            hash_from_input(true, || Ok("\n".to_owned()), &mut Cursor::new("")).expect_err("empty");
        assert!(format!("{empty:#}").contains("Leeres Passwort"));
        let broken = hash_from_input(
            true,
            || Err(std::io::Error::other("kein Terminal")),
            &mut Cursor::new(""),
        )
        .expect_err("unreadable");
        assert!(format!("{broken:#}").contains("Passwort nicht lesbar"));
    }
}
