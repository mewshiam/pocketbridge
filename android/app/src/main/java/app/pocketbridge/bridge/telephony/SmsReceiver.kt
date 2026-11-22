package app.pocketbridge.bridge.telephony

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.provider.Telephony
import android.telephony.SmsMessage
import app.pocketbridge.bridge.data.OfflineQueue

/**
 * SmsReceiver — incoming SMS relay.
 *
 * Every incoming SMS lands in the SQLCipher offline queue first (durability
 * before delivery): when the iOS client is connected the queue flushes
 * immediately, otherwise entries wait up to 7 days and flush in
 * chronological order on reconnect (protocol/SPEC.md §6).
 */
class SmsReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Telephony.Sms.Intents.SMS_RECEIVED_ACTION) return
        val messages = Telephony.Sms.Intents.getMessagesFromIntent(intent) ?: return
        if (messages.isEmpty()) return

        val body = messages.joinToString("") { it.displayMessageBody ?: "" }
        val address = messages.firstOrNull()?.originatingAddress ?: return
        val timestamp = messages.firstOrNull()?.timestampMillis ?: System.currentTimeMillis()
        val subId = messages.firstOrNull()?.let { msg ->
            try {
                msg.javaClass.getMethod("getSubscriptionId").invoke(msg) as? Int ?: 0
            } catch (e: Exception) {
                0
            }
        } ?: 0

        // Route into the encrypted queue; flush happens from the service.
        SmsReceiver.queue?.enqueueIncomingSms(
            address = address,
            body = body,
            timestampMs = timestamp,
            subscriptionId = subId,
        )
    }

    companion object {
        /** Set by BridgeForegroundService; JVM tests swap in an in-memory queue. */
        @Volatile
        var queue: OfflineQueue? = null
    }

    /** Test-visible extraction helper (pure function). */
    fun extractBody(messages: Array<SmsMessage>): String =
        messages.joinToString("") { it.displayMessageBody ?: "" }
}
