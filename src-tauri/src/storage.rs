use std::collections::HashMap;
use std::path::PathBuf;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use crate::domain::{AlertRule, Settings, HistoryPoint};
use crate::alerts::AlertState;

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
        conn.execute(
            "CREATE TABLE IF NOT EXISTS alert_rules (
                account_id TEXT NOT NULL,
                tier_id TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 0,
                threshold_percent INTEGER NOT NULL CHECK(threshold_percent BETWEEN 1 AND 99),
                PRIMARY KEY(account_id, tier_id),
                FOREIGN KEY(account_id) REFERENCES accounts(id) ON DELETE CASCADE
            );",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS alert_states (
                account_id TEXT NOT NULL,
                tier_id TEXT NOT NULL,
                last_resets_at INTEGER,
                last_utilization REAL NOT NULL,
                threshold_notified INTEGER NOT NULL DEFAULT 0,
                exhausted_notified INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY(account_id, tier_id),
                FOREIGN KEY(account_id, tier_id) REFERENCES alert_rules(account_id, tier_id) ON DELETE CASCADE
            );",
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
        let account_order = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'account_order'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
            .unwrap_or_default();
        Ok(Settings {
            refresh_interval_minutes: minutes,
            account_order,
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

    pub fn update_account_order(&self, order: &[String]) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        // Vec<String> always serializes; fall back to empty array on unexpected failure.
        let value = serde_json::to_string(order).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('account_order', ?)",
            [value],
        )?;
        Ok(())
    }

    pub fn save_account(
        &self,
        id: &str,
        provider: &str,
        display_name: &str,
        enabled: bool,
        credential_source: &str,
        config_json: &str,
        alert_rules: Option<&[AlertRule]>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let now = chrono::Utc::now().timestamp_millis();
        tx.execute(
            "INSERT INTO accounts (id, provider, display_name, enabled, credential_source, config_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                provider = excluded.provider,
                display_name = excluded.display_name,
                enabled = excluded.enabled,
                credential_source = excluded.credential_source,
                config_json = excluded.config_json,
                updated_at = excluded.updated_at",
            params![
                id,
                provider,
                display_name,
                if enabled { 1 } else { 0 },
                credential_source,
                config_json,
                now,
                now
            ],
        )?;

        if let Some(rules) = alert_rules {
            // Load existing rules to detect enabled/threshold changes.
            let mut existing: std::collections::HashMap<String, (bool, u8)> =
                std::collections::HashMap::new();
            {
                let mut stmt = tx.prepare(
                    "SELECT tier_id, enabled, threshold_percent FROM alert_rules WHERE account_id = ?",
                )?;
                let rows = stmt.query_map([id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i32>(1)? != 0,
                        row.get::<_, i32>(2)? as u8,
                    ))
                })?;
                for r in rows {
                    let (tier_id, en, thr) = r?;
                    existing.insert(tier_id, (en, thr));
                }
            }

            let mut keep_ids = std::collections::HashSet::new();
            for rule in rules {
                let tier_id = rule.tier_id.trim();
                keep_ids.insert(tier_id.to_string());

                if let Some((old_enabled, old_threshold)) = existing.get(tier_id) {
                    if *old_enabled != rule.enabled || *old_threshold != rule.threshold_percent {
                        tx.execute(
                            "DELETE FROM alert_states WHERE account_id = ? AND tier_id = ?",
                            params![id, tier_id],
                        )?;
                    }
                }

                tx.execute(
                    "INSERT INTO alert_rules (account_id, tier_id, enabled, threshold_percent)
                     VALUES (?, ?, ?, ?)
                     ON CONFLICT(account_id, tier_id) DO UPDATE SET
                        enabled = excluded.enabled,
                        threshold_percent = excluded.threshold_percent",
                    params![
                        id,
                        tier_id,
                        if rule.enabled { 1 } else { 0 },
                        rule.threshold_percent as i32
                    ],
                )?;
            }

            // Delete rules that were not submitted (cascades states).
            let stale: Vec<String> = existing
                .keys()
                .filter(|k| !keep_ids.contains(*k))
                .cloned()
                .collect();
            for tier_id in stale {
                tx.execute(
                    "DELETE FROM alert_rules WHERE account_id = ? AND tier_id = ?",
                    params![id, tier_id],
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn list_alert_rules(&self, account_id: &str) -> Result<Vec<AlertRule>, rusqlite::Error> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT tier_id, enabled, threshold_percent FROM alert_rules WHERE account_id = ? ORDER BY tier_id",
        )?;
        let rows = stmt.query_map([account_id], |row| {
            Ok(AlertRule {
                tier_id: row.get(0)?,
                enabled: row.get::<_, i32>(1)? != 0,
                threshold_percent: row.get::<_, i32>(2)? as u8,
            })
        })?;
        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn get_alert_state(
        &self,
        account_id: &str,
        tier_id: &str,
    ) -> Result<Option<AlertState>, rusqlite::Error> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT account_id, tier_id, last_resets_at, last_utilization, threshold_notified, exhausted_notified
             FROM alert_states WHERE account_id = ? AND tier_id = ?",
            params![account_id, tier_id],
            |row| {
                Ok(AlertState {
                    account_id: row.get(0)?,
                    tier_id: row.get(1)?,
                    last_resets_at: row.get(2)?,
                    last_utilization: row.get(3)?,
                    threshold_notified: row.get::<_, i32>(4)? != 0,
                    exhausted_notified: row.get::<_, i32>(5)? != 0,
                })
            },
        )
        .optional()
    }

    pub fn upsert_alert_state(&self, state: &AlertState) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO alert_states (
                account_id, tier_id, last_resets_at, last_utilization, threshold_notified, exhausted_notified
             ) VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(account_id, tier_id) DO UPDATE SET
                last_resets_at = excluded.last_resets_at,
                last_utilization = excluded.last_utilization,
                threshold_notified = excluded.threshold_notified,
                exhausted_notified = excluded.exhausted_notified",
            params![
                state.account_id,
                state.tier_id,
                state.last_resets_at,
                state.last_utilization,
                if state.threshold_notified { 1 } else { 0 },
                if state.exhausted_notified { 1 } else { 0 },
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
