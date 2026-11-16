# LinkJobs

**Share a link → a Rust background job fetches the page → the title shows up as a notification.**

A [Rivium](https://github.com/rivium-rs/rivium) app runs an [Apalis](https://github.com/apalis-dev/apalis)
worker with a SQLite queue. The same Rust core runs in three places:

| Where | How Rivium hosts it | How jobs get in | How results come out |
|---|---|---|---|
| Laptop | process host (`cargo run`) | `curl` | `curl` / logs |
| **Android** | embedded host via `rivium_jni::export!` | **Share Sheet** ("Fetch title") | notification + list |
| iOS (extra) | embedded host via a small C ABI | paste, `linkjobs://add?url=…`, Shortcuts action | notification + list |

The original challenge brief is in [CHALLENGE.md](CHALLENGE.md).

## How it works

```
 Share Sheet / paste / curl
            │  POST /jobs {"text": "...https://..."}
            ▼
 ┌──────────────── Rivium host (process / JNI / C ABI) ────────────────┐
 │  "api" service (frontline)        "worker" service (background)     │
 │  rivium-http + axum on            Apalis worker, 2 concurrent jobs,  │
 │  127.0.0.1:47800  ──push──▶ SQLite ◀──poll── retries w/ backoff     │
 │                         <root>/jobs.sqlite        │                  │
 └───────────────────────────────────────────────────┼──────────────────┘
                                                     ▼
                                         GET page, parse <title>
            GET /jobs ◀── results stored in SQLite ──┘
            ▼
 app polls → notification "Rust Programming Language"
```

* **One job type**, `FetchTitle { url }` (`crates/linkjobs-core/src/job.rs`). It reads at most
  512 KB of the page and returns `{ title, final_url, http_status, bytes_read }`. 5xx responses
  and network errors are retried 3 times with exponential backoff.
* **The phone apps don't link SQLite or Apalis themselves.** They talk to the core over a
  loopback HTTP API, so Android (Kotlin) and iOS (Swift) use the same code path:

  | Method | Path | |
  |---|---|---|
  | `POST` | `/jobs` | `{"text": "anything containing a link"}` → `201 {"url": ...}` |
  | `GET` | `/jobs` | latest 50 jobs, newest first |
  | `DELETE` | `/jobs` | remove finished jobs |
  | `GET` | `/stats` | `{"pending", "running", "done", "failed"}` |

* **Jobs survive closing the app.** They live in `<root>/jobs.sqlite` (the app's private files
  dir on Android, Application Support on iOS). Pending jobs run on the next start. Jobs that
  were *running* when the app was killed are re-enqueued by apalis-sqlite's orphan recovery,
  about 10s after the next start (5s heartbeat × 2).

### Layout

```
crates/linkjobs-core   Rivium App: Apalis worker + HTTP API + SQLite   (all the logic)
crates/linkjobs-cli    `linkjobs` binary: Rivium process host for the laptop
crates/linkjobs-jni    Android cdylib: rivium_jni::export!(class = "dev/linkjobs/RustBridge")
crates/linkjobs-ios    iOS staticlib: C ABI over rivium_host::embedded::Host + header/modulemap
android/               Android Studio project (Kotlin, no AndroidX dependencies)
ios/                   Xcode project (SwiftUI)
scripts/               build-rust-android.sh, build-rust-ios.sh
```

## Run it

You need Rust 1.92+ (`rustup`). Tested with 1.100.0.

### 1. Laptop

```sh
cargo run -p linkjobs-cli -- --root target/linkjobs
```

In another terminal:

```sh
curl -X POST localhost:47800/jobs -H 'content-type: application/json' \
     -d '{"text": "look at this https://www.rust-lang.org/"}'
curl localhost:47800/jobs      # a second later: "title": "Rust Programming Language"
```

Ctrl-C shuts down gracefully. Try `kill -9` while jobs are queued, then start again: they're
still there and run. Other Rivium commands work too: `-- default-config`, `--set concurrency=4`.

Tests: `cargo test --workspace`.

### 2. Android

1. Install Android Studio. In **Settings → SDK Manager → SDK Tools**, install
   **NDK (Side by side) 27.2.12479018** (or change `ndkVersion` in `android/app/build.gradle.kts`).
2. Open the `android/` folder in Android Studio and let Gradle sync. Accept the AGP upgrade
   assistant if it offers one.
3. Press **Run**. The `buildRust` Gradle task runs `scripts/build-rust-android.sh`, which
   installs `cargo-ndk` if needed and builds `liblinkjobs_jni.so` for `arm64-v8a` (phones) and
   `x86_64` (emulator) into `app/src/main/jniLibs/`.
4. Allow notifications. In Chrome (or any app), tap **Share → Fetch title**. You get a toast
   right away and a notification with the page title a moment later. Tap it to open the page.

### 3. iOS (Xcode)

```sh
open ios/LinkJobs.xcodeproj
```

Pick an iPhone simulator and press **Run** (⌘R). The **Build Rust core** build phase runs
`scripts/build-rust-ios.sh`, which compiles `crates/linkjobs-ios` for the selected
simulator or device (installing the Rust target the first time), so Rust changes are picked up
on every build. The first build takes a minute or two. For a real iPhone, set your Team under
*Signing & Capabilities* and change the bundle ID if it's taken.

Ways to add a job on iOS:
* Paste a link in the app.
* `xcrun simctl openurl booted 'linkjobs://add?url=https://example.com'`
* The **Fetch Page Title** action in the Shortcuts app. Turn on *Show in Share Sheet* for a
  shortcut that uses it, and you can share links from Safari.
* Because the simulator shares the Mac's network, `curl localhost:47800/...` works against the
  app running in the simulator.

## Versions

| | Version |
|---|---|
| rivium / rivium-host / rivium-http / rivium-jni | **0.2.0** (crates.io, pinned `=0.2.0`) |
| apalis | **1.2.0** (pinned) |
| apalis-sqlite | **1.1.0** (pinned), sqlx 0.9.0, bundled SQLite (libsqlite3-sys 0.37.0) |
| reqwest | 0.12 with rustls + ring (no OpenSSL / aws-lc, so it cross-compiles easily) |
| Rust | 1.100.0 stable (MSRV 1.92 because of Rivium) |
| Xcode | 27.2, iOS 17.0+ deployment target, tested on the iOS 27.2 simulator (iPhone 18 Pro) |
| Android | AGP 8.7.3, Gradle 8.11.1, Kotlin 2.0.21, NDK 27.2, minSdk 26, targetSdk 35 |

## Findings for the maintainers

* **apalis-sqlite: restarting quickly after a crash fails with `WORKER_ALREADY_EXISTS`.**
  If the process is killed (very common on phones) and restarted before the old worker's
  `last_seen` goes stale, registering the same worker name fails and the worker service dies.
  Workaround in `crates/linkjobs-core/src/lib.rs`: a unique worker name per run, so the dead
  run's jobs are re-enqueued as orphans.
* **apalis-sqlite: no default polling.** A `SqliteStorage` without a poll strategy never picks
  up jobs pushed from another handle. `poll_with_interval` fixes it.
* **apalis-file-storage 1.2.0: `FileStorage::new` opens with `truncate(true)`**, so all jobs are
  wiped on restart. That's why this project uses apalis-sqlite.
* **Rivium:** `App::services` runs outside the Tokio runtime, so anything that needs a runtime
  (like `sqlx` `connect_lazy`) must be created inside the service future.
* **Rivium on iOS:** the embedded host, file logging, `rivium-http` and the whole dependency
  tree build and run unchanged on `aarch64-apple-ios-sim`. Only a ~60-line C ABI
  (`crates/linkjobs-ios`) was needed in place of `rivium-jni`.

## Real-device testing

_TODO: fill in after running on a physical phone (bonus points)._

| Device | OS | Result |
|---|---|---|
| | | |
