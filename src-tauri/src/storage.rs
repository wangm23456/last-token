use std::collections::HashMap;
use std::path::PathBuf;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use crate::domain::{Settings, HistoryPoint};

// ── SecretStore Trait & Implementations ───────────────────────────

pub trait SecretStore: Send + Sync {
    fn get_secret(&self, account_id: &str, secret_name: &str) -> Result<Option<String>, String>;
    fn set_secret(&self, account_id: &str, secret_name: &str, value: &str) -> Result<(), String>;
    fn delete_secret(&self, account_id: &str, secret_name: &str) -> Result<(), String>;
}

pub struct InMemorySecretStore {
    secrets: Mutex<HashMap<String, String>>,
}

impl InMemorySecretStore {
    pub fn new() -> Self {
        Self {
            secrets: Mutex::new(HashMap::new()),
        }
    }
}

impl SecretStore for InMemorySecretStore {
    fn get_secret(&self, account_id: &str, secret_name: &str) -> Result<Option<String>, String> {
        let key = format!("account/{}/{}", account_id, secret_name);
        let map = self.secrets.lock();
        Ok(map.get(&key).cloned())
    }

    fn set_secret(&self, account_id: &str, secret_name: &str, value: &str) -> Result<(), String> {
        let key = format!("account/{}/{}", account_id, secret_name);
        let mut map = self.secrets.lock();
        map.insert(key, value.to_string());
        Ok(())
    }

    fn delete_secret(&self, account_id: &str, secret_name: &str) -> Result<(), String> {
        let key = format!("account/{}/{}", account_id, secret_name);
        let mut map = self.secrets.lock();
        map.remove(&key);
        Ok(())
    }
}

// ── SQLite Storage ──────────────────────────────────────────────

pub struct Storage {
    conn: Mutex<Connection>,
}

/// Full row of the latest stored sample for one (account, tier).
/// Internal only — the public contract stays HistoryPoint.
pub struct StoredQuotaSnapshot {
    pub utilization: f64,
    pub resets_at: Option<i64>,
    pub sampled_at: i64,
    pub used: Option<f64>,
    pub limit_value: Option<f64>,
    pub unit: Option<String>,
    pub unlimited: bool,
}

impl Storage {
    pub fn lock_conn(&self) -> parking_lot::MutexGuard<'_, Connection> {
        self.conn.lock()
    }

    pub fn new(db_path: PathBuf) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(db_path)?;
        conn.execute("PRAGMA foreign_keys = ON;", [])?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        
        let s = Self {
            conn: Mutex::new(conn),
        };
        s.init_schema()?;
        Ok(s)
    }

    fn init_schema(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS accounts (
                id TEXT PRIMARY KEY,
                provider TEXT NOT NULL,
                display_name TEXT NOT NULL,
                enabled INTEGER NOT NULL,
                credential_source TEXT NOT NULL,
                config_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS quota_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                tier_id TEXT NOT NULL,
                utilization REAL NOT NULL,
                resets_at INTEGER,
                sampled_at INTEGER NOT NULL,
                used REAL,
                limit_value REAL,
                unit TEXT,
                unlimited INTEGER NOT NULL DEFAULT 0
            );",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_quota_snapshots ON quota_snapshots (account_id, tier_id, sampled_at DESC);",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
            [],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES ('refresh_interval_minutes', '5');",
            [],
        )?;
        // Migrate legacy keyring accounts to env-based source after keyring support was removed.
        conn.execute(
            "UPDATE OR IGNORE accounts SET credential_source = 'env' WHERE credential_source = 'keyring';",
            [],
        )?;
        Ok(())
    }

    pub fn get_settings(&self) -> Result<Settings, rusqlite::Error> {
        let conn = self.conn.lock();
        let val: String = conn.query_row(
            "SELECT value FROM settings WHERE key = 'refresh_interval_minutes'",
            [],
            |row| row.get(0),
        )?;
        let minutes: i64 = val.parse().unwrap_or(5);
        Ok(Settings {
            refresh_interval_minutes: minutes,
        })
    }

    pub fn update_settings(&self, minutes: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('refresh_interval_minutes', ?)",
            [minutes.to_string()],
        )?;
        Ok(())
    }

    pub fn save_account(&self, id: &str, provider: &str, display_name: &str, enabled: bool, credential_source: &str, config_json: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT OR REPLACE INTO accounts (id, provider, display_name, enabled, credential_source, config_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, COALESCE((SELECT created_at FROM accounts WHERE id = ?), ?), ?)",
            params![
                id,
                provider,
                display_name,
                if enabled { 1 } else { 0 },
                credential_source,
                config_json,
                id,
                now,
                now
            ],
        )?;
        Ok(())
    }

    pub fn delete_account(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM accounts WHERE id = ?", [id])?;
        Ok(())
    }

    pub fn get_account_raw(&self, id: &str) -> Result<Option<(String, String, String, bool, String, String)>, rusqlite::Error> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, provider, display_name, enabled, credential_source, config_json FROM accounts WHERE id = ?",
            [id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i32>(3)? != 0,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()
    }

    pub fn list_accounts_raw(&self) -> Result<Vec<(String, String, String, bool, String, String)>, rusqlite::Error> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT id, provider, display_name, enabled, credential_source, config_json FROM accounts")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i32>(3)? != 0,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;
        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn insert_snapshot(
        &self,
        account_id: &str,
        tier_id: &str,
        utilization: f64,
        resets_at: Option<i64>,
        sampled_at: i64,
        used: Option<f64>,
        limit_value: Option<f64>,
        unit: Option<&str>,
        unlimited: bool,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO quota_snapshots (account_id, tier_id, utilization, resets_at, sampled_at, used, limit_value, unit, unlimited)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                account_id,
                tier_id,
                utilization,
                resets_at,
                sampled_at,
                used,
                limit_value,
                unit,
                if unlimited { 1 } else { 0 }
            ],
        )?;
        Ok(())
    }

    pub fn get_snapshots(&self, account_id: &str, tier_id: &str, limit: usize) -> Result<Vec<HistoryPoint>, rusqlite::Error> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT sampled_at, utilization, resets_at FROM quota_snapshots
             WHERE account_id = ? AND tier_id = ?
             ORDER BY sampled_at DESC LIMIT ?",
        )?;
        let rows = stmt.query_map(params![account_id, tier_id, limit], |row| {
            Ok(HistoryPoint {
                sampled_at: row.get(0)?,
                utilization: row.get(1)?,
                resets_at: row.get(2)?,
            })
        })?;
        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        // Return in chronological order
        results.reverse();
        Ok(results)
    }

    /// Tier IDs belonging to the most recent successful batch for this account.
    /// All tiers written by one refresh share the same `sampled_at`, so this
    /// keeps every window the upstream returned last time and drops tiers
    /// that only exist in older batches.
    pub fn get_current_tier_ids(&self, account_id: &str) -> Result<Vec<String>, rusqlite::Error> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT tier_id FROM quota_snapshots
             WHERE account_id = ? AND sampled_at = (
                 SELECT MAX(sampled_at) FROM quota_snapshots WHERE account_id = ?
             )",
        )?;
        let rows = stmt.query_map(params![account_id, account_id], |row| row.get::<_, String>(0))?;
        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    /// Latest stored sample with full quota metadata for one (account, tier).
    pub fn get_latest_snapshot(
        &self,
        account_id: &str,
        tier_id: &str,
    ) -> Result<Option<StoredQuotaSnapshot>, rusqlite::Error> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT utilization, resets_at, sampled_at, used, limit_value, unit, unlimited
             FROM quota_snapshots
             WHERE account_id = ? AND tier_id = ?
             ORDER BY sampled_at DESC LIMIT 1",
            params![account_id, tier_id],
            |row| {
                Ok(StoredQuotaSnapshot {
                    utilization: row.get(0)?,
                    resets_at: row.get(1)?,
                    sampled_at: row.get(2)?,
                    used: row.get(3)?,
                    limit_value: row.get(4)?,
                    unit: row.get(5)?,
                    unlimited: row.get::<_, i32>(6)? != 0,
                })
            },
        )
        .optional()
    }

    pub fn clear_history(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM quota_snapshots", [])?;
        Ok(())
    }

    pub fn prune_snapshots(&self, max_age_ms: i64) -> Result<usize, rusqlite::Error> {
        let conn = self.conn.lock();
        let pruned = conn.execute("DELETE FROM quota_snapshots WHERE sampled_at < ?", [max_age_ms])?;
        Ok(pruned)
    }
}
