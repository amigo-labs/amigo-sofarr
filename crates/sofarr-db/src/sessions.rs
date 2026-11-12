//! Login sessions (spec auth).

use jiff::Timestamp;

use crate::{Db, Result};

/// A stored session, found by the hash of its token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub fingerprint: String,
    pub created_at: Timestamp,
    pub last_used_at: Timestamp,
}

fn ts(raw: &str) -> Timestamp {
    raw.parse().unwrap_or(Timestamp::UNIX_EPOCH)
}

impl Db {
    pub async fn add_session(
        &self,
        token_hash: &str,
        fingerprint: &str,
        now: Timestamp,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO sessions (token_hash, fingerprint, created_at, last_used_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(token_hash)
        .bind(fingerprint)
        .bind(now.to_string())
        .bind(now.to_string())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn session(&self, token_hash: &str) -> Result<Option<Session>> {
        let row: Option<(String, String, String)> = sqlx::query_as(
            "SELECT fingerprint, created_at, last_used_at FROM sessions WHERE token_hash = ?",
        )
        .bind(token_hash)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(|(fingerprint, created, used)| Session {
            fingerprint,
            created_at: ts(&created),
            last_used_at: ts(&used),
        }))
    }

    pub async fn touch_session(&self, token_hash: &str, now: Timestamp) -> Result<()> {
        sqlx::query("UPDATE sessions SET last_used_at = ? WHERE token_hash = ?")
            .bind(now.to_string())
            .bind(token_hash)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn remove_session(&self, token_hash: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(token_hash)
            .execute(self.pool())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sessions_round_trip() {
        let db = Db::in_memory().await.expect("db");
        let t0: Timestamp = "2026-11-12T08:00:00Z".parse().expect("ts");
        db.add_session("h1", "fp", t0).await.expect("add");
        assert_eq!(db.session("h2").await.expect("read"), None);
        let t1: Timestamp = "2026-11-13T08:00:00Z".parse().expect("ts");
        db.touch_session("h1", t1).await.expect("touch");
        let session = db.session("h1").await.expect("read").expect("session");
        assert_eq!(
            (
                session.fingerprint.as_str(),
                session.created_at,
                session.last_used_at
            ),
            ("fp", t0, t1)
        );
        db.remove_session("h1").await.expect("remove");
        assert_eq!(db.session("h1").await.expect("read"), None);
    }
}
