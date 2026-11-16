package dev.linkjobs

import android.app.Application
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.net.Uri
import android.util.Log
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/**
 * Starts the Rust core (Rivium embedded host + Apalis worker) once per process and watches
 * for finished jobs, posting a notification with the page title for each one.
 */
class LinkJobsApp : Application() {
    private val executor = Executors.newSingleThreadScheduledExecutor()
    private val started = CountDownLatch(1)
    @Volatile var startError: String? = null
        private set

    override fun onCreate() {
        super.onCreate()
        createChannel()
        executor.execute(::startCore)
        executor.scheduleWithFixedDelay(::checkFinished, 2, 1, TimeUnit.SECONDS)
    }

    /** Block (off the UI thread) until the Rust core has started or failed. */
    fun awaitCore(timeoutSeconds: Long = 30): Boolean =
        started.await(timeoutSeconds, TimeUnit.SECONDS) && startError == null

    private fun startCore() {
        val root = File(filesDir, "linkjobs").apply { mkdirs() }
        val code = RustBridge.nativeStart(arrayOf("--root", root.absolutePath))
        if (code != 0 && code != 1) {
            startError = "Rust core failed to start ($code): ${RustBridge.nativeLastError()}"
            Log.e(TAG, startError!!)
        } else {
            Log.i(TAG, "Rust core ${RustBridge.nativeVersion()} running, data in $root")
        }
        started.countDown()
    }

    private fun checkFinished() {
        if (startError != null || RustBridge.nativeStatus() != RustBridge.STATUS_RUNNING) return
        val jobs = runCatching { JobsApi.jobs() }.getOrElse { return }
        val prefs = getSharedPreferences("notified", MODE_PRIVATE)
        val seen = prefs.getStringSet("ids", emptySet())!!.toMutableSet()
        val fresh = jobs.filter { it.isFinished && it.id !in seen }
        if (fresh.isEmpty()) return
        fresh.forEach(::notify)
        seen += fresh.map { it.id }
        // Keep the set bounded to the jobs the API still returns.
        seen.retainAll(jobs.map { it.id }.toSet())
        prefs.edit().putStringSet("ids", seen).apply()
    }

    private fun notify(job: Job) {
        val target = Uri.parse(job.finalUrl ?: job.url)
        val open = PendingIntent.getActivity(
            this, job.id.hashCode(), Intent(Intent.ACTION_VIEW, target),
            PendingIntent.FLAG_IMMUTABLE,
        )
        val (title, text) = if (job.status == "Done") {
            (job.title ?: "No title found") to (job.finalUrl ?: job.url)
        } else {
            "Couldn't fetch link" to "${job.url}\n${job.error ?: job.status}"
        }
        val notification = Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_link)
            .setContentTitle(title)
            .setContentText(text)
            .setStyle(Notification.BigTextStyle().bigText(text))
            .setContentIntent(open)
            .setAutoCancel(true)
            .build()
        // Silently skipped if the user denied POST_NOTIFICATIONS; MainActivity still lists results.
        runCatching { getSystemService(NotificationManager::class.java).notify(job.id.hashCode(), notification) }
    }

    private fun createChannel() {
        val channel = NotificationChannel(CHANNEL, "Fetched links", NotificationManager.IMPORTANCE_DEFAULT)
        getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
    }

    companion object {
        const val TAG = "LinkJobs"
        private const val CHANNEL = "results"
    }
}
