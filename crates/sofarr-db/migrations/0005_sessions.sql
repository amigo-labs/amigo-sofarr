-- Login sessions (spec auth): only the SHA-256 of a token is stored. The fingerprint of
-- username and password hash ends every session when either changes.

CREATE TABLE sessions (
    token_hash   TEXT PRIMARY KEY,
    fingerprint  TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    last_used_at TEXT NOT NULL
);
