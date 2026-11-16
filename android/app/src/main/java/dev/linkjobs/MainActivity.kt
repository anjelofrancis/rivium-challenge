package dev.linkjobs

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Typeface
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.View
import android.view.ViewGroup
import android.widget.ArrayAdapter
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ListView
import android.widget.TextView
import kotlin.concurrent.thread

/** Lists jobs from the Rust core, refreshing every second while visible. */
class MainActivity : Activity() {
    private val handler = Handler(Looper.getMainLooper())
    private lateinit var status: TextView
    private lateinit var adapter: JobAdapter
    private val refresher = object : Runnable {
        override fun run() {
            refresh()
            handler.postDelayed(this, 1000)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val pad = (16 * resources.displayMetrics.density).toInt()

        val input = EditText(this).apply { hint = "Paste a link"; setSingleLine() }
        val add = Button(this).apply { text = "Fetch" }
        val clear = Button(this).apply { text = "Clear finished" }
        status = TextView(this).apply { text = "Starting Rust core…"; setPadding(0, pad / 2, 0, pad / 2) }
        adapter = JobAdapter()
        val list = ListView(this).apply {
            adapter = this@MainActivity.adapter
            emptyView = null
            setOnItemClickListener { _, _, position, _ ->
                val job = this@MainActivity.adapter.getItem(position)!!
                startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(job.finalUrl ?: job.url)))
            }
        }

        add.setOnClickListener {
            val text = input.text.toString()
            input.text.clear()
            background { JobsApi.enqueue(text); null }
        }
        clear.setOnClickListener { background { JobsApi.clearFinished(); null } }

        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            addView(input, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
            addView(add)
        }
        setContentView(LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(pad, pad, pad, 0)
            addView(TextView(context).apply {
                text = "Share any link to \"Fetch title\" from another app, or paste one here."
            })
            addView(row)
            addView(status)
            addView(clear)
            addView(list, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
        })

        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1)
        }
    }

    override fun onResume() {
        super.onResume()
        handler.post(refresher)
    }

    override fun onPause() {
        handler.removeCallbacks(refresher)
        super.onPause()
    }

    private fun refresh() {
        val app = application as LinkJobsApp
        thread {
            val result = runCatching { JobsApi.jobs() }
            runOnUiThread {
                result.onSuccess { jobs ->
                    val counts = jobs.groupingBy { it.status }.eachCount()
                    status.text = "Rust core running · " +
                        counts.entries.joinToString { "${it.value} ${it.key.lowercase()}" }.ifEmpty { "no jobs yet" }
                    adapter.clear()
                    adapter.addAll(jobs)
                }.onFailure {
                    status.text = app.startError ?: "Waiting for Rust core…"
                }
            }
        }
    }

    private fun background(work: () -> String?) {
        thread {
            val error = try {
                (application as LinkJobsApp).awaitCore()
                work()
            } catch (e: Exception) {
                e.message
            }
            runOnUiThread {
                error?.let { status.text = it }
                refresh()
            }
        }
    }

    private inner class JobAdapter : ArrayAdapter<Job>(this, android.R.layout.simple_list_item_2, android.R.id.text1) {
        override fun getView(position: Int, convertView: View?, parent: ViewGroup): View {
            val view = super.getView(position, convertView, parent)
            val job = getItem(position)!!
            val icon = when (job.status) {
                "Done" -> "✅"
                "Failed", "Killed" -> "❌"
                "Running", "Queued" -> "⏳"
                else -> "🕒"
            }
            view.findViewById<TextView>(android.R.id.text1).apply {
                text = "$icon ${job.title ?: job.url}"
                setTypeface(null, Typeface.BOLD)
            }
            view.findViewById<TextView>(android.R.id.text2).text =
                if (job.status == "Done") job.url else "${job.status} · ${job.error ?: job.url}"
            return view
        }
    }
}
