package app.pocketbridge.bridge.data

/**
 * OfflineQueue — SQLCipher-backed durability for undelivered traffic.
 *
 * Blueprint §5.6: Android persists undelivered incoming SMS and missed-call
 * events in SQLCipher (up to 7 days) and flushes chronologically on
 * reconnect. This interface is the contract; the production implementation
 * binds SQLCipher on device, tests use [InMemoryOfflineQueue].
 */
interface OfflineQueue {

    /** A queue entry waiting for delivery to the paired client. */
    data class Entry(
        val kind: Kind,
        val payload: String,
        val address: String,
        val timestampMs: Long,
        val subscriptionId: Int,
    )

    enum class Kind { INCOMING_SMS, MISSED_CALL }

    /** Adds an incoming SMS for later delivery. */
    fun enqueueIncomingSms(address: String, body: String, timestampMs: Long, subscriptionId: Int)

    /** Adds a missed-call event for later delivery. */
    fun enqueueMissedCall(phoneNumber: String, contactName: String, timestampMs: Long, subscriptionId: Int)

    /** Chronologically ordered entries, oldest first. */
    fun snapshot(): List<Entry>

    /** Removes entries delivered (acked) by the client. */
    fun removeDelivered(timestamps: List<Long>)

    /** Drops entries older than the 7-day retention window; returns count. */
    fun evictStale(nowMs: Long): Int

    /** Total entries waiting. */
    fun size(): Int
}

/** 7-day retention (protocol/SPEC.md §6). */
const val RETENTION_MS: Long = 7L * 24 * 60 * 60 * 1000

/**
 * In-memory implementation used by JVM tests and as the semantic reference
 * for the SQLCipher-backed production queue.
 */
class InMemoryOfflineQueue : OfflineQueue {

    private val entries = mutableListOf<OfflineQueue.Entry>()

    override fun enqueueIncomingSms(address: String, body: String, timestampMs: Long, subscriptionId: Int) {
        entries.add(
            OfflineQueue.Entry(
                kind = OfflineQueue.Kind.INCOMING_SMS,
                payload = body,
                address = address,
                timestampMs = timestampMs,
                subscriptionId = subscriptionId,
            )
        )
    }

    override fun enqueueMissedCall(phoneNumber: String, contactName: String, timestampMs: Long, subscriptionId: Int) {
        entries.add(
            OfflineQueue.Entry(
                kind = OfflineQueue.Kind.MISSED_CALL,
                payload = "$phoneNumber|$contactName",
                address = phoneNumber,
                timestampMs = timestampMs,
                subscriptionId = subscriptionId,
            )
        )
    }

    override fun snapshot(): List<OfflineQueue.Entry> =
        entries.sortedBy { it.timestampMs }

    override fun removeDelivered(timestamps: List<Long>) {
        entries.removeAll { it.timestampMs in timestamps }
    }

    override fun evictStale(nowMs: Long): Int {
        val cutoff = nowMs - RETENTION_MS
        val before = entries.size
        entries.removeAll { it.timestampMs < cutoff }
        return before - entries.size
    }

    override fun size(): Int = entries.size
}
