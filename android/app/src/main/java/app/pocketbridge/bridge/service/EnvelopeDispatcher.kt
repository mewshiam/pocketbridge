package app.pocketbridge.bridge.service

import app.pocketbridge.bridge.telephony.SmsSender
import app.pocketbridge.bridge.telephony.SmsReceiver

/**
 * EnvelopeDispatcher — routes decrypted control-plane traffic.
 *
 * In the full build the bytes arriving here are serialized `Envelope`
 * protobuf messages; this scaffold implements the heartbeat subset
 * (Ping -> Pong + DeviceStatus) and forwards SMS/call traffic to the
 * telephony layer. Keeping the dispatch in one pure function makes the
 * JVM unit tests trivial (no Android framework classes needed).
 */
object EnvelopeDispatcher {

    /** A control message to (re)send over CTRL_TX. */
    data class Reply(val plaintext: ByteArray, val handshake: Boolean)

    /** Parses a decrypted message and returns the reply bytes, if any. */
    fun dispatch(bytes: ByteArray, send: (ByteArray, Boolean) -> Unit) {
        val text = bytes.decodeToString()
        when {
            text.startsWith("ping:") -> {
                val seq = text.removePrefix("ping:")
                send("pong:$seq".encodeToByteArray(), false)
                send(deviceStatus().encodeToByteArray(), false)
            }
            text.startsWith("sms:") -> {
                // SendSmsRequest: "sms:<address>|<body>|<subscriptionId>"
                val parts = text.removePrefix("sms:").split("|")
                if (parts.size >= 2) {
                    val subId = parts.getOrNull(2)?.toIntOrNull() ?: 0
                    val result = SmsSender.send(parts[0], parts[1], subId)
                    send("smsresult:$result".encodeToByteArray(), false)
                }
            }
            else -> Unit // Envelope payloads beyond the heartbeat subset
        }
    }

    /** DeviceStatus summary line (battery, SIMs, audio strategy). */
    fun deviceStatus(): String {
        val battery = BatteryStatus.read()
        val sims = SimStatus.read()
        val strategy = AudioStrategyStatus.activeStrategyName()
        return "status:battery=${battery.first},charging=${battery.second},sims=$sims,strategy=$strategy"
    }
}

/** Battery and SIM status probes; overridable in tests. */
object BatteryStatus {
    @Volatile
    var probe: () -> Pair<Int, Boolean> = { 100 to false }

    fun read(): Pair<Int, Boolean> = probe()
}

object SimStatus {
    @Volatile
    var probe: () -> String = { "sim0:carrier=A,signal=4" }

    fun read(): String = probe()
}

object AudioStrategyStatus {
    @Volatile
    var probe: () -> String = { "native-voice-capture" }

    fun activeStrategyName(): String = probe()
}
