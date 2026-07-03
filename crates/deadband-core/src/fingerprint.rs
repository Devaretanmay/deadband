use std::path::PathBuf;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoopFingerprint {
    /// SHA-256 hash of the canonical trajectory (sorted tool+args pairs)
    pub hash: String,
    /// JSON-serialized list of `(tool_name, stripped_args)` pairs
    pub trajectory_json: String,
    /// Human-readable trajectory summary (e.g. "search -> search -> search")
    pub trajectory_summary: String,
    /// First time this fingerprint was seen
    pub first_seen: Option<DateTime<Utc>>,
    /// Most recent time this fingerprint was seen
    pub last_seen: Option<DateTime<Utc>>,
    /// Total number of times this loop pattern has been detected
    pub occurrence_count: u64,
    /// The intervention that was applied (if any)
    pub last_intervention: Option<String>,
    /// The execution IDs associated with this fingerprint
    pub execution_ids: Vec<Uuid>,
}

pub struct FingerprintStore {
    conn: Connection,
    db_path: PathBuf,
}

impl FingerprintStore {
    /// Open (or create) the fingerprint database at the given path.
    pub fn open(db_path: impl Into<PathBuf>) -> Result<Self, rusqlite::Error> {
        let db_path = db_path.into();
        let conn = Connection::open(&db_path)?;
        let store = Self { conn, db_path };
        store.initialize_tables()?;
        Ok(store)
    }

    /// Open the default fingerprint database at `./deadband_fingerprints.db`.
    pub fn open_default() -> Result<Self, rusqlite::Error> {
        Self::open("deadband_fingerprints.db")
    }

    /// Open an in-memory fingerprint database (useful for testing).
    pub fn open_in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn,
            db_path: PathBuf::from(":memory:"),
        };
        store.initialize_tables()?;
        Ok(store)
    }

    fn initialize_tables(&self) -> Result<(), rusqlite::Error> {
        // Remove the GROUP_CONCAT length limit to avoid silent truncation
        // of execution ID lists for sessions with many fingerprint matches.
        self.conn.execute_batch("PRAGMA group_concat_max_len = -1;")?;
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS loop_fingerprints (
                hash TEXT PRIMARY KEY,
                trajectory_json TEXT NOT NULL,
                trajectory_summary TEXT NOT NULL,
                first_seen TEXT NOT NULL,
                last_seen TEXT NOT NULL,
                occurrence_count INTEGER NOT NULL DEFAULT 1,
                last_intervention TEXT
            );

            CREATE TABLE IF NOT EXISTS fingerprint_executions (
                hash TEXT NOT NULL,
                execution_id TEXT NOT NULL,
                seen_at TEXT NOT NULL,
                FOREIGN KEY (hash) REFERENCES loop_fingerprints(hash)
            );

            CREATE INDEX IF NOT EXISTS idx_fingerprint_executions_hash
                ON fingerprint_executions(hash);

            CREATE INDEX IF NOT EXISTS idx_fingerprint_executions_execution_id
                ON fingerprint_executions(execution_id);",
        )?;
        Ok(())
    }

    /// Compute a SHA-256 fingerprint from a trajectory of (tool_name, args) pairs.
    /// The args should already be canonicalized (volatile fields stripped).
    pub fn compute_fingerprint(trajectory: &[(String, String)]) -> String {
        let mut hasher = Sha256::new();
        for (tool, args) in trajectory {
            hasher.update(tool.as_bytes());
            hasher.update(b":");
            hasher.update(args.as_bytes());
            hasher.update(b"|");
        }
        hex::encode(hasher.finalize())
    }

    /// Store or update a fingerprint for a loop detection.
    pub fn record_fingerprint(
        &self,
        fingerprint: &str,
        trajectory: &[(String, String)],
        execution_id: Uuid,
        intervention: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let summary = trajectory
            .iter()
            .map(|(t, _)| t.as_str())
            .collect::<Vec<_>>()
            .join(" -> ");

        let trajectory_json =
            serde_json::to_string(trajectory).unwrap_or_default();
        let now = Utc::now().to_rfc3339();

        // Check if fingerprint already exists
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT hash FROM loop_fingerprints WHERE hash = ?1",
                params![fingerprint],
                |row| row.get(0),
            )
            .ok();

        if existing.is_some() {
            // Update existing fingerprint
            self.conn.execute(
                "UPDATE loop_fingerprints
                 SET last_seen = ?1,
                     occurrence_count = occurrence_count + 1,
                     last_intervention = COALESCE(?2, last_intervention)
                 WHERE hash = ?3",
                params![now, intervention, fingerprint],
            )?;
        } else {
            // Insert new fingerprint
            self.conn.execute(
                "INSERT INTO loop_fingerprints (hash, trajectory_json, trajectory_summary, first_seen, last_seen, occurrence_count, last_intervention)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
                params![fingerprint, trajectory_json, summary, now, now, intervention],
            )?;
        }

        // Record the execution association
        self.conn.execute(
            "INSERT INTO fingerprint_executions (hash, execution_id, seen_at)
             VALUES (?1, ?2, ?3)",
            params![fingerprint, execution_id.to_string(), now],
        )?;

        Ok(())
    }

    /// Look up a fingerprint by hash.
    pub fn lookup(&self, hash: &str) -> Result<Option<LoopFingerprint>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT f.hash, f.trajectory_json, f.trajectory_summary,
                    f.first_seen, f.last_seen, f.occurrence_count, f.last_intervention,
                    GROUP_CONCAT(fe.execution_id, ',') as execution_ids
             FROM loop_fingerprints f
             LEFT JOIN fingerprint_executions fe ON f.hash = fe.hash
             WHERE f.hash = ?1
             GROUP BY f.hash",
        )?;

        let mut results = stmt.query_map(params![hash], |row| {
            let first_seen_str: String = row.get(3)?;
            let last_seen_str: String = row.get(4)?;
            let exec_ids_str: String = row.get(7)?;

            let execution_ids: Vec<Uuid> = if exec_ids_str.is_empty() {
                Vec::new()
            } else {
                exec_ids_str
                    .split(',')
                    .filter_map(|s| Uuid::parse_str(s).ok())
                    .collect()
            };

            Ok(LoopFingerprint {
                hash: row.get(0)?,
                trajectory_json: row.get(1)?,
                trajectory_summary: row.get(2)?,
                first_seen: DateTime::parse_from_rfc3339(&first_seen_str)
                    .ok()
                    .map(|d| d.with_timezone(&Utc)),
                last_seen: DateTime::parse_from_rfc3339(&last_seen_str)
                    .ok()
                    .map(|d| d.with_timezone(&Utc)),
                occurrence_count: row.get::<_, i64>(5)? as u64,
                last_intervention: row.get(6)?,
                execution_ids,
            })
        })?;

        match results.next() {
            Some(Ok(fp)) => Ok(Some(fp)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    /// List all fingerprints, ordered by most recent first.
    pub fn list_recent(&self, limit: usize) -> Result<Vec<LoopFingerprint>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT f.hash, f.trajectory_json, f.trajectory_summary,
                    f.first_seen, f.last_seen, f.occurrence_count, f.last_intervention,
                    GROUP_CONCAT(fe.execution_id, ',') as execution_ids
             FROM loop_fingerprints f
             LEFT JOIN fingerprint_executions fe ON f.hash = fe.hash
             GROUP BY f.hash
             ORDER BY f.last_seen DESC
             LIMIT ?1",
        )?;

        let results = stmt.query_map(params![limit as i64], |row| {
            let first_seen_str: String = row.get(3)?;
            let last_seen_str: String = row.get(4)?;
            let exec_ids_str: String = row.get(7)?;

            let execution_ids: Vec<Uuid> = if exec_ids_str.is_empty() {
                Vec::new()
            } else {
                exec_ids_str
                    .split(',')
                    .filter_map(|s| Uuid::parse_str(s).ok())
                    .collect()
            };

            Ok(LoopFingerprint {
                hash: row.get(0)?,
                trajectory_json: row.get(1)?,
                trajectory_summary: row.get(2)?,
                first_seen: DateTime::parse_from_rfc3339(&first_seen_str)
                    .ok()
                    .map(|d| d.with_timezone(&Utc)),
                last_seen: DateTime::parse_from_rfc3339(&last_seen_str)
                    .ok()
                    .map(|d| d.with_timezone(&Utc)),
                occurrence_count: row.get::<_, i64>(5)? as u64,
                last_intervention: row.get(6)?,
                execution_ids,
            })
        })?;

        results.collect::<Result<Vec<_>, _>>()
    }

    /// Get the number of unique fingerprints stored.
    pub fn count(&self) -> Result<u64, rusqlite::Error> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM loop_fingerprints",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c as u64)
    }

    /// Database path for diagnostics.
    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_fingerprint() {
        let trajectory = vec![
            ("search".to_string(), r#"{"q":"hello"}"#.to_string()),
            ("search".to_string(), r#"{"q":"hello"}"#.to_string()),
        ];
        let hash1 = FingerprintStore::compute_fingerprint(&trajectory);

        let trajectory2 = vec![
            ("search".to_string(), r#"{"q":"hello"}"#.to_string()),
            ("search".to_string(), r#"{"q":"hello"}"#.to_string()),
        ];
        let hash2 = FingerprintStore::compute_fingerprint(&trajectory2);
        assert_eq!(hash1, hash2);

        let trajectory3 = vec![
            ("search".to_string(), r#"{"q":"world"}"#.to_string()),
            ("search".to_string(), r#"{"q":"world"}"#.to_string()),
        ];
        let hash3 = FingerprintStore::compute_fingerprint(&trajectory3);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_record_and_lookup() {
        let store = FingerprintStore::open_in_memory().unwrap();
        let trajectory = vec![
            ("read_file".to_string(), r#"{"path":"/tmp/x"}"#.to_string()),
            ("read_file".to_string(), r#"{"path":"/tmp/x"}"#.to_string()),
        ];
        let hash = FingerprintStore::compute_fingerprint(&trajectory);
        let exec_id = Uuid::new_v4();

        store
            .record_fingerprint(&hash, &trajectory, exec_id, Some("abort"))
            .unwrap();

        let fp = store.lookup(&hash).unwrap().unwrap();
        assert_eq!(fp.hash, hash);
        assert_eq!(fp.occurrence_count, 1);
        assert_eq!(fp.last_intervention, Some("abort".to_string()));
    }

    #[test]
    fn test_record_updates_existing() {
        let store = FingerprintStore::open_in_memory().unwrap();
        let trajectory = vec![
            ("api_call".to_string(), r#"{}"#.to_string()),
        ];
        let hash = FingerprintStore::compute_fingerprint(&trajectory);
        let exec_id = Uuid::new_v4();

        store
            .record_fingerprint(&hash, &trajectory, exec_id, Some("retry"))
            .unwrap();
        store
            .record_fingerprint(&hash, &trajectory, Uuid::new_v4(), Some("abort"))
            .unwrap();

        let fp = store.lookup(&hash).unwrap().unwrap();
        assert_eq!(fp.occurrence_count, 2);
        assert_eq!(fp.execution_ids.len(), 2);
    }

    #[test]
    fn test_list_recent() {
        let store = FingerprintStore::open_in_memory().unwrap();
        let traj1 = vec![("tool_a".to_string(), "{}".to_string())];
        let traj2 = vec![("tool_b".to_string(), "{}".to_string())];
        let hash1 = FingerprintStore::compute_fingerprint(&traj1);
        let hash2 = FingerprintStore::compute_fingerprint(&traj2);

        store
            .record_fingerprint(&hash2, &traj2, Uuid::new_v4(), None)
            .unwrap();
        store
            .record_fingerprint(&hash1, &traj1, Uuid::new_v4(), None)
            .unwrap();

        let recent = store.list_recent(10).unwrap();
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn test_count() {
        let store = FingerprintStore::open_in_memory().unwrap();
        assert_eq!(store.count().unwrap(), 0);

        let traj = vec![("tool".to_string(), "{}".to_string())];
        let hash = FingerprintStore::compute_fingerprint(&traj);
        store
            .record_fingerprint(&hash, &traj, Uuid::new_v4(), None)
            .unwrap();
        assert_eq!(store.count().unwrap(), 1);
    }
}
