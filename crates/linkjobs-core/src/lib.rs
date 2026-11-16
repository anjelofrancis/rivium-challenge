//! LinkJobs: a Rivium application that runs an Apalis worker.
//!
//! Two supervised services share one SQLite database under the Rivium `--root`:
//!
//! * `worker` (background): an Apalis worker that runs [`job::fetch_title`] jobs.
//! * `api` (frontline): a loopback HTTP API the phone apps use to queue jobs and read results.
//!
//! The same [`LinkJobs`] app runs as a laptop process (`linkjobs-cli`), inside an Android app
//! (`linkjobs-jni`) and inside an iOS app (`linkjobs-ios`).

pub mod api;
pub mod db;
pub mod job;

use std::{net::SocketAddr, time::Duration};

use apalis::layers::retry::{
    HasherRng,
    backoff::{ExponentialBackoffMaker, MakeBackoff},
};
use apalis::prelude::*;
use rivium::{Error, Kind, Result, Service, service};
use rivium_host::{App, Context};
use rivium_http::HttpServer;
use serde::{Deserialize, Serialize};

use crate::db::Db;

/// Port the phone apps connect to on 127.0.0.1.
pub const DEFAULT_PORT: u16 = 47800;

const POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Loopback address of the job API.
    pub listen: SocketAddr,
    /// How many jobs run at the same time.
    pub concurrency: usize,
    /// Attempts per job before it is marked Failed.
    pub retries: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: ([127, 0, 0, 1], DEFAULT_PORT).into(),
            concurrency: 2,
            retries: 3,
        }
    }
}

pub struct LinkJobs;

impl App for LinkJobs {
    const NAME: &'static str = "linkjobs";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    type Config = Config;

    fn validate(config: &Config) -> Result<()> {
        if !(1..=16).contains(&config.concurrency) {
            return Err(Error::invalid_input("concurrency must be 1..=16"));
        }
        if config.retries > 10 {
            return Err(Error::invalid_input("retries must be 0..=10"));
        }
        Ok(())
    }

    fn services(config: &Config, context: &Context<Config>) -> Result<Vec<Box<dyn Service>>> {
        let db = Db::open(&context.paths().root().join("jobs.sqlite"));
        Ok(vec![
            worker_service(db.clone(), config.concurrency, config.retries),
            Box::new(HttpServer::new("api", config.listen, api::router(db))),
        ])
    }
}

fn worker_service(db: Db, concurrency: usize, retries: usize) -> Box<dyn Service> {
    service("worker", Kind::Background, move |ctx| async move {
        let storage = db.storage().await.map_err(Error::internal)?;
        let client = job::http_client().map_err(|e| Error::internal(e.to_string()))?;
        let backoff = ExponentialBackoffMaker::new(
            Duration::from_secs(1),
            Duration::from_secs(10),
            2.0,
            HasherRng::default(),
        )
        .map_err(|e| Error::internal(e.to_string()))?
        .make_backoff();

        // A unique name per run: after the app is force-killed the previous worker's row
        // still looks alive, and apalis-sqlite refuses to re-register the same name
        // (WORKER_ALREADY_EXISTS). With a fresh name, the old worker's jobs are re-enqueued
        // by orphan recovery instead.
        let name = format!("{}-{}", db::QUEUE, unix_millis());
        let worker = WorkerBuilder::new(name)
            // Poll SQLite once a second for jobs queued by the API (or left from a previous run).
            .backend(storage.poll_with_interval(POLL_INTERVAL))
            .data(client)
            .concurrency(concurrency)
            .retry(RetryPolicy::retries(retries).with_backoff(backoff))
            .build(job::fetch_title);

        ctx.ready();
        tracing::info!("worker ready");
        worker
            .run_until(async move {
                ctx.stopped().await;
                Ok::<(), std::io::Error>(())
            })
            .await
            .map_err(|e| Error::internal(e.to_string()))?;
        tracing::info!("worker stopped");
        Ok(())
    })
}

fn unix_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}
