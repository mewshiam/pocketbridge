package app.pocketbridge.bridge

import app.pocketbridge.bridge.data.CallLogSyncProvider
import app.pocketbridge.bridge.data.ContactsSyncProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SyncProviderUnitTest {
    @Test
    fun `contacts sync paginates and filters by timestamp`() {
        val contacts = (1..250).map { i ->
            ContactsSyncProvider.Contact("c$i", "Name $i", listOf("+1555$i"), updatedAtMs = i.toLong())
        }
        val pages = ContactsSyncProvider.batches(contacts, sinceMs = 0, pageSize = 100)
        assertEquals(3, pages.size)
        assertEquals(250, pages.sumOf { it.size })
        // incremental: only newer entries
        assertEquals(50, ContactsSyncProvider.batches(contacts, sinceMs = 200).sumOf { it.size })
    }

    @Test
    fun `call log sync is ordered and limited`() {
        val entries = (1..10).map { i ->
            CallLogSyncProvider.Entry("l$i", "+1555$i", "OUTGOING", dateMs = i * 100L, durationSec = i, subscriptionId = 0)
        }
        val got = CallLogSyncProvider.since(entries, sinceMs = 300, limit = 5)
        assertEquals(listOf(400L, 500L, 600L, 700L, 800L), got.map { it.dateMs })
        assertEquals(5, CallLogSyncProvider.since(entries, 0, 5).size)
        assertTrue(CallLogSyncProvider.since(entries, 9999).isEmpty())
    }
}
