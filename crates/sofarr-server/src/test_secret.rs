//! Passwords for tests, made at runtime: none is written into the code (code scanning
//! flags hard-coded passwords, also in tests).

use std::sync::LazyLock;

use rand::RngCore;

/// A fresh random secret.
pub fn secret() -> String {
    let mut bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The test user's password, the same within one test run.
pub fn password() -> &'static str {
    static PASSWORD: LazyLock<String> = LazyLock::new(secret);
    PASSWORD.as_str()
}

/// A wrong password, the same within one test run and never the right one.
pub fn wrong() -> &'static str {
    static WRONG: LazyLock<String> = LazyLock::new(|| format!("x{}", secret()));
    WRONG.as_str()
}
