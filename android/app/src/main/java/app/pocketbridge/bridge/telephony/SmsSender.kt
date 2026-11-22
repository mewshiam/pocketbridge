package app.pocketbridge.bridge.telephony

import android.content.Context
import android.telephony.SmsManager
import android.util.Log

/**
 * SmsSender — dual-SIM-aware outgoing SMS.
 *
 * subscriptionId 0 selects the default SIM; a nonzero id is routed through
 * the platform's [SmsManager] for that subscription. The send result feeds
 * `SendSmsResult` envelopes back to the iOS client.
 */
object SmsSender {

    /** Result of one send attempt, mapped into SendSmsResult. */
    data class Result(val success: Boolean, val messageId: String?, val errorReason: String?)

    interface Backend {
        fun sendText(address: String, body: String, subscriptionId: Int): Boolean
    }

    /** Default backend talks to the Android telephony stack. */
    @Volatile
    var backend: (Context?) -> Backend = { ctx ->
        PlatformBackend(ctx)
    }

    fun send(address: String, body: String, subscriptionId: Int): String {
        val result = sendWithResult(address, body, subscriptionId)
        return if (result.success) {
            "ok:${result.messageId ?: ""}"
        } else {
            "error:${result.errorReason ?: "unknown"}"
        }
    }

    fun sendWithResult(address: String, body: String, subscriptionId: Int, context: Context? = null): Result {
        if (address.isBlank()) {
            return Result(false, null, "empty address")
        }
        if (body.isEmpty()) {
            return Result(false, null, "empty body")
        }
        val ok = try {
            backend(context).sendText(address, body, subscriptionId)
        } catch (e: Exception) {
            Log.w(TAG, "sendText failed: ${e.message}")
            false
        }
        return if (ok) {
            Result(true, "sms-${System.currentTimeMillis()}", null)
        } else {
            Result(false, null, "send failed")
        }
    }

    private class PlatformBackend(private val context: Context?) : Backend {
        override fun sendText(address: String, body: String, subscriptionId: Int): Boolean {
            val manager = if (subscriptionId != 0 && android.os.Build.VERSION.SDK_INT >= 31) {
                SmsManager.getSmsManagerForSubscriptionId(subscriptionId)
            } else {
                // Default SIM: SmsManager.getDefault() on API < 31,
                // getSmsManagerForSubscriptionId(0) on newer APIs.
                if (android.os.Build.VERSION.SDK_INT >= 31) {
                    SmsManager.getSmsManagerForSubscriptionId(0)
                } else {
                    @Suppress("DEPRECATION")
                    SmsManager.getDefault()
                }
            }
            return try {
                manager.sendTextMessage(address, null, body, null, null)
                true
            } catch (e: Exception) {
                Log.w(TAG, "platform sendTextMessage failed: ${e.message}")
                false
            }
        }
    }

    private const val TAG = "PBSmsSender"
}
