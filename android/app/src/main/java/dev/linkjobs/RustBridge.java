package dev.linkjobs;

/**
 * Native methods registered by {@code rivium_jni::export!} in crates/linkjobs-jni.
 *
 * <p>Start and stop block while Rivium starts or drains services: call them off the UI thread.
 * Status: Idle=0, Starting=1, Running=2, Stopping=3, Restarting=4, Failed=5.
 * Results: 0 ok, 1 already running, 2 not running, 3 busy, 4 cancelled, negative = error.
 */
public final class RustBridge {
    static {
        System.loadLibrary("linkjobs_jni");
    }

    /** Port of the Rust job API on 127.0.0.1 (matches linkjobs_core::DEFAULT_PORT). */
    public static final int PORT = 47800;
    public static final int STATUS_RUNNING = 2;

    private RustBridge() {}

    static native int nativeStart(String[] args);

    static native int nativeStop(long timeoutMillis);

    static native int nativeStatus();

    static native String nativeLastError();

    static native String nativeVersion();
}
