// C interface to the LinkJobs Rust core (Rivium embedded host).
// Mirrors the five calls Rivium's Android JNI bridge exposes.
#ifndef LINKJOBS_H
#define LINKJOBS_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Status values returned by linkjobs_status().
#define LINKJOBS_IDLE 0
#define LINKJOBS_STARTING 1
#define LINKJOBS_RUNNING 2
#define LINKJOBS_STOPPING 3
#define LINKJOBS_RESTARTING 4
#define LINKJOBS_FAILED 5

// Result codes: 0 ok, 1 already running, 2 not running, 3 busy, 4 cancelled,
// negative values are errors (see linkjobs_last_error).

// Start the host with data under `root` (UTF-8 path). Blocks until services are ready,
// so call it off the main thread.
int32_t linkjobs_start(const char *root);

// Gracefully stop, waiting up to `timeout_ms`. Blocks; call off the main thread.
int32_t linkjobs_stop(uint64_t timeout_ms);

int32_t linkjobs_status(void);

// Port of the loopback job API (http://127.0.0.1:<port>).
uint16_t linkjobs_port(void);

// Last error / version strings. Free with linkjobs_string_free. May return NULL.
char *linkjobs_last_error(void);
char *linkjobs_version(void);
void linkjobs_string_free(char *s);

#ifdef __cplusplus
}
#endif

#endif
