package dev.linkjobs

import org.json.JSONArray
import org.json.JSONObject
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL

/** Mirrors linkjobs_core::db::JobView. */
data class Job(
    val id: String,
    val url: String,
    val status: String,
    val title: String?,
    val finalUrl: String?,
    val error: String?,
) {
    val isFinished get() = status == "Done" || status == "Failed" || status == "Killed"
}

/** Blocking client for the Rust job API on 127.0.0.1. Call from a background thread. */
object JobsApi {
    private const val BASE = "http://127.0.0.1:${RustBridge.PORT}"

    class ApiException(message: String) : IOException(message)

    /** Queue a job for the first link in [text]. Returns the normalised URL. */
    fun enqueue(text: String): String {
        val body = JSONObject().put("text", text).toString()
        return JSONObject(request("POST", "/jobs", body)).getString("url")
    }

    fun jobs(): List<Job> {
        val array = JSONArray(request("GET", "/jobs"))
        return (0 until array.length()).map { i ->
            val o = array.getJSONObject(i)
            val result = o.optJSONObject("result")
            Job(
                id = o.getString("id"),
                url = o.getString("url"),
                status = o.getString("status"),
                title = result?.optStringOrNull("title"),
                finalUrl = result?.optStringOrNull("final_url"),
                error = o.optStringOrNull("error"),
            )
        }
    }

    fun clearFinished() {
        request("DELETE", "/jobs")
    }

    private fun request(method: String, path: String, body: String? = null): String {
        val conn = URL(BASE + path).openConnection() as HttpURLConnection
        try {
            conn.requestMethod = method
            conn.connectTimeout = 2000
            conn.readTimeout = 5000
            if (body != null) {
                conn.doOutput = true
                conn.setRequestProperty("Content-Type", "application/json")
                conn.outputStream.use { it.write(body.toByteArray()) }
            }
            val code = conn.responseCode
            val stream = if (code in 200..299) conn.inputStream else conn.errorStream
            val text = stream?.bufferedReader()?.use { it.readText() } ?: ""
            if (code !in 200..299) {
                val description = runCatching { JSONObject(text).getString("description") }.getOrNull()
                throw ApiException(description ?: "HTTP $code")
            }
            return text
        } finally {
            conn.disconnect()
        }
    }

    private fun JSONObject.optStringOrNull(key: String): String? =
        if (isNull(key)) null else optString(key).ifEmpty { null }
}
