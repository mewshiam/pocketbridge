package app.pocketbridge.bridge.data

/**
 * ContactsSyncProvider — incremental contact sync (feature flag 0x0004).
 * Produces paginated batches ordered by updated_at for ContactsSyncBatch.
 */
object ContactsSyncProvider {
    data class Contact(val contactId: String, val displayName: String, val phoneNumbers: List<String>, val updatedAtMs: Long)

    /** Pages contacts updated after `sinceMs` into batches of `pageSize`. */
    fun batches(contacts: List<Contact>, sinceMs: Long, pageSize: Int = 100): List<List<Contact>> {
        val sorted = contacts.filter { it.updatedAtMs > sinceMs }.sortedBy { it.updatedAtMs }
        if (sorted.isEmpty()) return emptyList()
        return sorted.chunked(pageSize.coerceAtLeast(1))
    }
}
