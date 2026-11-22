package app.pocketbridge.bridge

import app.pocketbridge.bridge.data.InMemoryOfflineQueue
import app.pocketbridge.bridge.data.OfflineQueue
import app.pocketbridge.bridge.service.BatteryStatus
import app.pocketbridge.bridge.service.EnvelopeDispatcher
import app.pocketbridge.bridge.service.SimStatus
import app.pocketbridge.bridge.telephony.SmsSender
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * JVM unit tests for the bridge MVP (blueprint Task 3):
 * mocked GATT control-plane exchanges for handshake + Ping/Pong +
 * DeviceStatus, plus offline queue ordering and dual-SIM SMS routing.
 */
class BridgeUnitTest {

    // --- Heartbeat: Ping -> Pong + DeviceStatus --------------------------

    @Test
    fun `ping produces pong and device status`() {
        val replies = mutableListOf<ByteArray>()
        EnvelopeDispatcher.dispatch("ping:42".encodeToByteArray()) { bytes, handshake ->
            replies += bytes
            assertTrue(!handshake)
        }
        assertEquals(2, replies.size)
        assertEquals("pong:42", replies[0].decodeToString())
        assertTrue(replies[1].decodeToString().startsWith("status:battery="))
    }

    @Test
    fun `device status reflects battery, sims, strategy`() {
        BatteryStatus.probe = { 67 to true }
        SimStatus.probe = { "sim0:carrier=Irancell,signal=3;sim1:carrier=HamraheAval,signal=2" }
        val status = EnvelopeDispatcher.deviceStatus()
        assertTrue(status.contains("battery=67"))
        assertTrue(status.contains("charging=true"))
        assertTrue(status.contains("sims=sim0:carrier=Irancell"))
        assertTrue(status.contains("strategy=native-voice-capture"))
    }

    // --- SMS request routing ----------------------------------------------

    @Test
    fun `sms request routes through sender and reports result`() {
        val sent = mutableListOf<Triple<String, String, Int>>()
        SmsSender.backend = { _ ->
            object : SmsSender.Backend {
                override fun sendText(address: String, body: String, subscriptionId: Int): Boolean {
                    sent += Triple(address, body, subscriptionId)
                    return true
                }
            }
        }
        val replies = mutableListOf<ByteArray>()
        EnvelopeDispatcher.dispatch("sms:+15551234567|hello there|1".encodeToByteArray()) { bytes, _ ->
            replies += bytes
        }
        assertEquals(listOf(Triple("+15551234567", "hello there", 1)), sent)
        assertEquals(1, replies.size)
        assertTrue(replies[0].decodeToString().startsWith("smsresult:ok:"))
    }

    @Test
    fun `sms send failure surfaces error reason`() {
        SmsSender.backend = { _ ->
            object : SmsSender.Backend {
                override fun sendText(address: String, body: String, subscriptionId: Int): Boolean = false
            }
        }
        val result = SmsSender.sendWithResult("+15550000000", "body", 0)
        assertTrue(!result.success)
        assertEquals("send failed", result.errorReason)
    }

    @Test
    fun `sms validation rejects blank address`() {
        val result = SmsSender.sendWithResult("  ", "body", 0)
        assertTrue(!result.success)
        assertEquals("empty address", result.errorReason)
    }

    @Test
    fun `sms validation rejects empty body`() {
        val result = SmsSender.sendWithResult("+1555", "", 0)
        assertTrue(!result.success)
        assertEquals("empty body", result.errorReason)
    }

    @Test
    fun `default subscription id falls back to zero`() {
        var lastSub = -1
        SmsSender.backend = { _ ->
            object : SmsSender.Backend {
                override fun sendText(address: String, body: String, subscriptionId: Int): Boolean {
                    lastSub = subscriptionId
                    return true
                }
            }
        }
        EnvelopeDispatcher.dispatch("sms:+1555|hi|garbage".encodeToByteArray()) { _, _ -> }
        assertEquals(0, lastSub)
    }

    // --- Offline queue ordering + retention -------------------------------

    @Test
    fun `queue flushes chronologically`() {
        val queue = InMemoryOfflineQueue()
        queue.enqueueIncomingSms("+15551", "late", 3_000, 0)
        queue.enqueueIncomingSms("+15552", "early", 1_000, 0)
        queue.enqueueIncomingSms("+15553", "middle", 2_000, 0)
        val order = queue.snapshot().map { it.payload }
        assertEquals(listOf("early", "middle", "late"), order)
    }

    @Test
    fun `queue retention drops entries older than 7 days`() {
        val queue = InMemoryOfflineQueue()
        val now = 1_000_000_000L
        queue.enqueueIncomingSms("+1", "fresh", now - 1000, 0)
        queue.enqueueIncomingSms("+2", "ancient", now - (7L * 24 * 60 * 60 * 1000) - 1, 0)
        assertEquals(1, queue.evictStale(now))
        assertEquals(1, queue.size())
        assertEquals("fresh", queue.snapshot().single().payload)
    }

    @Test
    fun `delivered entries are removed`() {
        val queue: OfflineQueue = InMemoryOfflineQueue()
        queue.enqueueIncomingSms("+1", "a", 100, 0)
        queue.enqueueIncomingSms("+2", "b", 200, 0)
        queue.removeDelivered(listOf(100L))
        assertEquals(listOf("b"), queue.snapshot().map { it.payload })
    }

    @Test
    fun `missed call entries queue with phone number and name`() {
        val queue = InMemoryOfflineQueue()
        queue.enqueueMissedCall("+15559998888", "Ada Lovelace", 5_000, 1)
        val entry = queue.snapshot().single()
        assertEquals(OfflineQueue.Kind.MISSED_CALL, entry.kind)
        assertEquals("+15559998888|Ada Lovelace", entry.payload)
        assertEquals(1, entry.subscriptionId)
    }
}
