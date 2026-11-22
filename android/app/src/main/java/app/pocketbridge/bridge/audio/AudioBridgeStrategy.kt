package app.pocketbridge.bridge.audio

/**
 * AudioBridgeStrategy — pluggable two-way call audio capture/injection.
 *
 * OEM ROMs vary wildly on whether `VOICE_COMMUNICATION`/`VOICE_CALL` audio
 * can be captured; PocketBridge probes four tiers and locks the best one via
 * the 30-second Audio Self-Test Wizard (blueprint §7.1). Ordering matters:
 * lower tier number = better quality/reliability.
 */
interface AudioBridgeStrategy {

    /** Tier number (1..4), stable identifier for DeviceStatus reporting. */
    val tier: Int

    /** Short machine name reported in DeviceStatus.activeAudioStrategy. */
    val name: String

    /** Human-readable label (localized in the UI layer). */
    val label: String

    /** Probe the strategy; returns the measured quality score 0..100. */
    fun selfTest(): Int

    /** Start routing call audio. */
    fun start()

    /** Stop routing call audio. */
    fun stop()
}

/** Tier 1 — AudioRecord/AudioTrack via VOICE_COMMUNICATION / VOICE_CALL. */
class NativeVoiceCaptureStrategy : AudioBridgeStrategy {
    override val tier = 1
    override val name = "native-voice-capture"
    override val label = "Native voice capture"

    override fun selfTest(): Int {
        // Full implementation opens AudioRecord(VOICE_COMMUNICATION) and
        // measures RMS + round-trip on the AudioTrack; scaffold returns a
        // deterministic probe result consumed by the wizard ordering.
        return 90
    }

    override fun start() = Unit
    override fun stop() = Unit
}

/** Tier 2 — internal Bluetooth HFP/SCO routing where firmware supports it. */
class BluetoothScoBridgeStrategy : AudioBridgeStrategy {
    override val tier = 2
    override val name = "bluetooth-sco-bridge"
    override val label = "Bluetooth SCO bridge"

    override fun selfTest(): Int = 65

    override fun start() = Unit
    override fun stop() = Unit
}

/** Tier 3 — optional opt-in Shizuku module (CAPTURE_AUDIO_OUTPUT). */
class ShizukuCaptureStrategy : AudioBridgeStrategy {
    override val tier = 3
    override val name = "shizuku-capture"
    override val label = "Shizuku capture (opt-in)"

    override fun selfTest(): Int = 75

    override fun start() = Unit
    override fun stop() = Unit
}

/** Tier 4 — low-volume speakerphone + mic loop, aggressive WebRTC AEC/NS. */
class AcousticFallbackStrategy : AudioBridgeStrategy {
    override val tier = 4
    override val name = "acoustic-fallback"
    override val label = "Acoustic fallback (last resort)"

    override fun selfTest(): Int = 40

    override fun start() = Unit
    override fun stop() = Unit
}

/**
 * The wizard: probes every strategy for ~30 seconds total and locks the
 * highest-scoring tier. Pure logic so it is unit-testable.
 */
object AudioSelfTestWizard {

    fun run(strategies: List<AudioBridgeStrategy>): AudioBridgeStrategy =
        strategies.maxBy { it.selfTest() }

    fun defaultStrategies(): List<AudioBridgeStrategy> = listOf(
        NativeVoiceCaptureStrategy(),
        BluetoothScoBridgeStrategy(),
        ShizukuCaptureStrategy(),
        AcousticFallbackStrategy(),
    )
}
