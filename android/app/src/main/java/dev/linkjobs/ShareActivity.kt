package dev.linkjobs

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import kotlin.concurrent.thread

/**
 * Invisible Share Sheet target. Sharing a link from any app queues a Rust job and returns
 * straight away; the title arrives later as a notification posted by [LinkJobsApp].
 */
class ShareActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val text = intent?.takeIf { it.action == Intent.ACTION_SEND }
            ?.getStringExtra(Intent.EXTRA_TEXT)
        if (text.isNullOrBlank()) {
            finish()
            return
        }
        val app = application as LinkJobsApp
        thread(name = "share-enqueue") {
            val message = try {
                if (!app.awaitCore()) app.startError ?: "Rust core is not running"
                else "Fetching title for ${JobsApi.enqueue(text)}"
            } catch (e: Exception) {
                e.message ?: "Could not queue link"
            }
            runOnUiThread {
                Toast.makeText(applicationContext, message, Toast.LENGTH_LONG).show()
                finish()
            }
        }
    }
}
