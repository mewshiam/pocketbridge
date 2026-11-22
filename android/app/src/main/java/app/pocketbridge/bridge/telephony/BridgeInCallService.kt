package app.pocketbridge.bridge.telephony

import android.telecom.InCallService
import android.telecom.Call

/**
 * BridgeInCallService — mirrors platform call state into CallStateEvent
 * envelopes (DIALING/RINGING/ACTIVE/HOLDING/DISCONNECTED) and applies
 * CallActionRequests (ANSWER/REJECT/HANGUP/...) from the iOS client.
 */
class BridgeInCallService : InCallService() {

    override fun onCallAdded(call: Call) {
        call.registerCallback(object : Call.Callback() {
            override fun onStateChanged(call: Call, state: Int) {
                val mapped = when (state) {
                    Call.STATE_RINGING -> "RINGING"
                    Call.STATE_ACTIVE -> "ACTIVE"
                    Call.STATE_DIALING -> "DIALING"
                    Call.STATE_HOLDING -> "HOLDING"
                    Call.STATE_DISCONNECTED -> "DISCONNECTED"
                    else -> "UNSPECIFIED"
                }
                StateHolder.lastEvent = "call:${call.details.handle ?: ""}:$mapped"
            }
        })
    }

    /** Test-visible last emitted event. */
    object StateHolder { @Volatile var lastEvent: String? = null }
}
