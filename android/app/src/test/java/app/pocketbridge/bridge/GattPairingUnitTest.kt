package app.pocketbridge.bridge

import app.pocketbridge.bridge.audio.AcousticFallbackStrategy
import app.pocketbridge.bridge.audio.AudioSelfTestWizard
import app.pocketbridge.bridge.audio.BluetoothScoBridgeStrategy
import app.pocketbridge.bridge.audio.NativeVoiceCaptureStrategy
import app.pocketbridge.bridge.audio.ShizukuCaptureStrategy
import app.pocketbridge.bridge.ble.BleAdvertiser
import app.pocketbridge.bridge.data.KeyStoreRepo
import app.pocketbridge.bridge.data.MemoryStore
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.security.SecureRandom

/**
 * GATT + pairing + audio-strategy unit tests (JVM, no Android framework).
 */
class GattPairingUnitTest {

    // --- GATT UUIDs match the wire protocol --------------------------------

    @Test
    fun `gatt uuids match protocol spec`() {
        assertEquals("8f2b0001-6a4e-4c1d-9b3a-1e5f7d2c8a01", app.pocketbridge.bridge.ble.GattServerManager.SERVICE_UUID.toString())
        assertEquals("8f2b0002-6a4e-4c1d-9b3a-1e5f7d2c8a01", app.pocketbridge.bridge.ble.GattServerManager.CTRL_TX_UUID.toString())
        assertEquals("8f2b0003-6a4e-4c1d-9b3a-1e5f7d2c8a01", app.pocketbridge.bridge.ble.GattServerManager.CTRL_RX_UUID.toString())
        assertEquals("8f2b0004-6a4e-4c1d-9b3a-1e5f7d2c8a01", app.pocketbridge.bridge.ble.GattServerManager.L2CAP_PSM_UUID.toString())
        assertEquals("8f2b0005-6a4e-4c1d-9b3a-1e5f7d2c8a01", app.pocketbridge.bridge.ble.GattServerManager.DEVICE_INFO_UUID.toString())
    }

    @Test
    fun `device info payload layout is major minor flags`() {
        val flags = app.pocketbridge.bridge.ble.GattServerManager.FLAG_L2CAP_VOICE or
            app.pocketbridge.bridge.ble.GattServerManager.FLAG_DUAL_SIM
        val payload = byteArrayOf(1, 0, (flags and 0xFF).toByte(), ((flags shr 8) and 0xFF).toByte())
        assertEquals(1, payload[0].toInt())
        assertEquals(0, payload[1].toInt())
        assertEquals(0x03, payload[2].toInt())
        assertEquals(0x00, payload[3].toInt())
    }

    // --- Advertiser short id ----------------------------------------------

    @Test
    fun `adv id is 8 uppercase hex chars`() {
        repeat(16) {
            val id = BleAdvertiser.generateAdvId()
            assertEquals(8, id.length)
            assertTrue(id.all { c -> c in '0'..'9' || c in 'A'..'F' })
        }
    }

    @Test
    fun `adv ids vary`() {
        val ids = (1..64).map { BleAdvertiser.generateAdvId() }.toSet()
        assertTrue("expected collisions to be rare", ids.size > 60)
    }

    // --- Keystore repo (in-memory store) ----------------------------------

    @Test
    fun `keystore repo roundtrips pinned peer key`() {
        val repo = KeyStoreRepo(MemoryStore())
        val peerKey = ByteArray(32) { it.toByte() }
        repo.savePinnedPeerKey(peerKey)
        assertTrue(repo.isPaired())
        assertTrue(repo.loadPinnedPeerKey()!!.contentEquals(peerKey))
    }

    @Test
    fun `keystore repo clear removes pairing`() {
        val repo = KeyStoreRepo(MemoryStore())
        repo.savePinnedPeerKey(ByteArray(32))
        repo.clearPairing()
        assertTrue(!repo.isPaired())
        assertEquals(null, repo.loadPinnedPeerKey())
    }

    @Test
    fun `local public key survives store roundtrip`() {
        val repo = KeyStoreRepo(MemoryStore())
        val pub = ByteArray(32) { (it * 3).toByte() }
        repo.saveLocalPublicKey(pub)
        assertTrue(repo.loadLocalPublicKey()!!.contentEquals(pub))
    }

    // --- Audio strategy wizard ---------------------------------------------

    @Test
    fun `wizard locks highest scoring strategy`() {
        val best = AudioSelfTestWizard.run(AudioSelfTestWizard.defaultStrategies())
        assertEquals(1, best.tier)
        assertEquals("native-voice-capture", best.name)
    }

    @Test
    fun `tier ordering is quality-ranked`() {
        val strategies = listOf(
            AcousticFallbackStrategy(),
            BluetoothScoBridgeStrategy(),
            ShizukuCaptureStrategy(),
            NativeVoiceCaptureStrategy(),
        )
        val sorted = strategies.sortedBy { it.tier }
        assertEquals(1, sorted.first().tier)
        assertEquals(4, sorted.last().tier)
    }

    @Test
    fun `every strategy reports a distinct name`() {
        val names = AudioSelfTestWizard.defaultStrategies().map { it.name }.toSet()
        assertEquals(4, names.size)
    }
}
