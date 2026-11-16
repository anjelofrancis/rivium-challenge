//! SQLite storage shared by the Apalis worker and the HTTP API.
//!
//! Jobs live in `<root>/jobs.sqlite`, so they survive the app being closed: pending jobs
//! are picked up again on the next start, and jobs that were running when the process died
//! are re-enqueued by apalis-sqlite's orphan recovery.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use apalis_sqlite::{Config, SqliteStorage};
use serde::Serialize;
use serde_json::Value;
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use tokio::sync::OnceCell;

use crate::job::{FetchTitle, PageInfo};

/// Apalis queue name (the `Jobs.job_type` column).
pub const QUEUE: &str = "fetch-title";

#[derive(Clone)]
pub struct Db {
    path: PathBuf,
    pool: Arc<OnceCell<SqlitePool>>,
}

/// One job as the phone apps see it.
#[derive(Debug, Serialize)]
pub struct JobView {
    pub id: String,
    pub url: String,
    /// Apalis status: Pending, Queued, Running, Done, Failed or Killed.
    pub status: String,
    pub attempts: i64,
    /// Unix seconds.
    pub created_at: i64,
    pub done_at: Option<i64>,
    pub result: Option<PageInfo>,
    pub error: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct Stats {
    pub pending: i64,
    pub running: i64,
    pub done: i64,
    pub failed: i64,
}

impl Db {
    /// Remember where the database lives. Nothing is opened until [`Db::ready`], because
    /// `App::services` runs outside the Tokio runtime and sqlx pools need one.
    pub fn open(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            pool: Arc::new(OnceCell::new()),
        }
    }

    /// Open the pool and run the Apalis migrations once; concurrent callers share the work.
    pub async fn ready(&self) -> Result<&SqlitePool, String> {
        self.pool
            .get_or_try_init(|| async {
                if let Some(dir) = self.path.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
                }
                let options = SqliteConnectOptions::new()
                    .filename(&self.path)
                    .create_if_missing(true);
                let pool = SqlitePoolOptions::new()
                    .max_connections(4)
                    .connect_with(options)
                    .await
                    .map_err(|e| format!("cannot open {}: {e}", self.path.display()))?;
                SqliteStorage::setup(&pool)
                    .await
                    .map_err(|e| format!("database setup failed: {e}"))?;
                // Each run registers a new worker name; drop rows from runs long gone.
                sqlx::query(
                    "DELETE FROM Workers WHERE worker_type = ?1 \
                     AND last_seen < strftime('%s', 'now') - 86400 \
                     AND id NOT IN (SELECT lock_by FROM Jobs WHERE lock_by IS NOT NULL)",
                )
                .bind(QUEUE)
                .execute(&pool)
                .await
                .map_err(|e| format!("worker cleanup failed: {e}"))?;
                Ok::<_, String>(pool)
            })
            .await
    }

    pub async fn storage(&self) -> Result<SqliteStorage<FetchTitle>, String> {
        let pool = self.ready().await?;
        Ok(SqliteStorage::new(pool).with_config(
            Config::default()
                .queue(QUEUE)
                // A phone app can be killed at any moment. With a short heartbeat, jobs that
                // were running when that happened are re-enqueued ~10s after the next start.
                .heartbeat_interval(Duration::from_secs(5))
                .missed_heartbeats(2),
        ))
    }

    pub async fn recent(&self, limit: i64) -> Result<Vec<JobView>, String> {
        let rows = sqlx::query(
            "SELECT id, job, status, attempts, run_at, done_at, last_result FROM Jobs \
             WHERE job_type = ?1 ORDER BY run_at DESC, rowid DESC LIMIT ?2",
        )
        .bind(QUEUE)
        .bind(limit)
        .fetch_all(self.ready().await?)
        .await
        .map_err(|e| e.to_string())?;
        Ok(rows.iter().map(row_to_view).collect())
    }

    pub async fn stats(&self) -> Result<Stats, String> {
        let rows = sqlx::query(
            "SELECT status, COUNT(*) AS n FROM Jobs WHERE job_type = ?1 GROUP BY status",
        )
        .bind(QUEUE)
        .fetch_all(self.ready().await?)
        .await
        .map_err(|e| e.to_string())?;
        let mut stats = Stats::default();
        for row in &rows {
            let n: i64 = row.get("n");
            match row.get::<String, _>("status").as_str() {
                "Pending" | "Queued" => stats.pending += n,
                "Running" => stats.running += n,
                "Done" => stats.done += n,
                _ => stats.failed += n,
            }
        }
        Ok(stats)
    }

    /// Delete finished jobs; pending and running ones are kept.
    pub async fn clear_finished(&self) -> Result<u64, String> {
        let result = sqlx::query(
            "DELETE FROM Jobs WHERE job_type = ?1 AND status IN ('Done', 'Failed', 'Killed')",
        )
        .bind(QUEUE)
        .execute(self.ready().await?)
        .await
        .map_err(|e| e.to_string())?;
        Ok(result.rows_affected())
    }
}

fn row_to_view(row: &sqlx::sqlite::SqliteRow) -> JobView {
    let args: Option<FetchTitle> = serde_json::from_slice(&row.get::<Vec<u8>, _>("job")).ok();
    // apalis-sqlite stores `Result<T, String>` as `{"Ok": T}` or `{"Err": "..."}`.
    let last: Option<Value> = row
        .get::<Option<String>, _>("last_result")
        .and_then(|text| serde_json::from_str(&text).ok());
    let (result, error) = match last {
        Some(Value::Object(mut map)) => (
            map.remove("Ok")
                .and_then(|ok| serde_json::from_value(ok).ok()),
            map.remove("Err").map(|err| match err {
                Value::String(s) => s,
                other => other.to_string(),
            }),
        ),
        _ => (None, None),
    };
    JobView {
        id: row.get("id"),
        url: args.map(|a| a.url).unwrap_or_default(),
        status: row.get("status"),
        attempts: row.get("attempts"),
        created_at: row.get("run_at"),
        done_at: row.get("done_at"),
        result,
        error,
    }
}
