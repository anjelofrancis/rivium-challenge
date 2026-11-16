//! Loopback HTTP API used by the Android and iOS apps (and `curl` on a laptop).
//!
//! | Method | Path    | Body / response                                   |
//! |--------|---------|---------------------------------------------------|
//! | POST   | /jobs   | `{"text": "anything containing a link"}` → 201    |
//! | GET    | /jobs   | latest 50 jobs, newest first                      |
//! | DELETE | /jobs   | remove finished jobs                              |
//! | GET    | /stats  | counts per state                                  |

use apalis::prelude::TaskSink;
use axum::{Json, Router, extract::State, routing::get};
use rivium::Error;
use rivium_http::{ApiResponse, ApiResult};
use serde::{Deserialize, Serialize};

use crate::{
    db::{Db, JobView, Stats},
    job::{FetchTitle, extract_url},
};

#[derive(Deserialize)]
struct CreateJob {
    /// A URL, or shared text containing one.
    text: String,
}

#[derive(Serialize)]
struct Queued {
    url: String,
}

#[derive(Serialize)]
struct Cleared {
    removed: u64,
}

pub fn router(db: Db) -> Router {
    Router::new()
        .route("/jobs", get(list).post(create).delete(clear))
        .route("/stats", get(stats))
        .with_state(db)
}

fn internal(message: String) -> Error {
    Error::internal(message)
}

async fn create(State(db): State<Db>, Json(body): Json<CreateJob>) -> ApiResult<Queued> {
    let url = extract_url(&body.text).ok_or_else(|| {
        Error::invalid_input("no http(s) link in shared text")
            .with_public_message("That doesn't contain an http(s) link.")
    })?;
    db.storage()
        .await
        .map_err(internal)?
        .push(FetchTitle { url: url.clone() })
        .await
        .map_err(|e| internal(e.to_string()))?;
    tracing::info!(%url, "job queued");
    Ok(ApiResponse::Created(Queued { url }))
}

async fn list(State(db): State<Db>) -> ApiResult<Vec<JobView>> {
    Ok(ApiResponse::Data(db.recent(50).await.map_err(internal)?))
}

async fn stats(State(db): State<Db>) -> ApiResult<Stats> {
    Ok(ApiResponse::Data(db.stats().await.map_err(internal)?))
}

async fn clear(State(db): State<Db>) -> ApiResult<Cleared> {
    let removed = db.clear_finished().await.map_err(internal)?;
    Ok(ApiResponse::Data(Cleared { removed }))
}
