package app.pocketbridge.bridge.data

/** CallLogSyncProvider — incremental call-log sync, ordered oldest first. */
object CallLogSyncProvider {
    data class Entry(val logId: String, val phoneNumber: String, val type: String, val dateMs: Long, val durationSec: Int, val subscriptionId: Int)

    fun since(entries: List<Entry>, sinceMs: Long, limit: Int = 200): List<Entry> =
        entries.filter { it.dateMs > sinceMs }.sortedBy { it.dateMs }.take(limit.coerceAtLeast(1))
}
