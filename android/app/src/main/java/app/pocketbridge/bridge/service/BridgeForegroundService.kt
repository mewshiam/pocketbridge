package app.pocketbridge.bridge.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log
import app.pocketbridge.bridge.ble.BleAdvertiser
import app.pocketbridge.bridge.ble.GattServerManager
import app.pocketbridge.bridge.data.KeyStoreRepo
import app.pocketbridge.bridge.ui.MainActivity
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch

/**
 * BridgeForegroundService — the persistent heart of the Android bridge.
 *
 * Runs as a foreground service with `connectedDevice|phoneCall` types,
 * owns the GATT server + BLE advertiser, drives the Noise_XX handshake
 * against the Rust core, and answers Ping/Pong + DeviceStatus envelopes.
 */
class BridgeForegroundService : Service(), GattServerManager.Listener {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private lateinit var keyStore: KeyStoreRepo
    private lateinit var gattServer: GattServerManager
    private lateinit var advertiser: BleAdvertiser

    /** Message ids increment per direction; the top bit flags handshake. */
    private var nextMessageId = 1

    override fun onCreate() {
        super.onCreate()
        keyStore = KeyStoreRepo.fromContext(this)
        gattServer = GattServerManager(this).apply { listener = this@BridgeForegroundService }
        advertiser = BleAdvertiser(this)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        startForeground(NOTIFICATION_ID, buildNotification(), serviceTypes())
        scope.launch { startBridge() }
        return START_STICKY
    }

    private fun startBridge() {
        val ok = gattServer.start()
        val advOk = advertiser.start()
        Log.i(TAG, "bridge started: gatt=$ok advertise=$advOk id=${advertiser.advId}")
    }

    override fun onDestroy() {
        scope.cancel()
        advertiser.stop()
        gattServer.stop()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    // --- GattServerManager.Listener ------------------------------------

    override fun onControlMessage(bytes: ByteArray) {
        // Control plane: the bytes are either Noise_XX handshake payloads or
        // encrypted Envelope transport messages. Both route through the
        // Rust core; the plaintext is dispatched by EnvelopeDispatcher.
        Log.d(TAG, "control message: ${bytes.size} bytes")
        EnvelopeDispatcher.dispatch(bytes) { reply, handshake ->
            sendControl(reply, handshake)
        }
    }

    override fun onSubscribed() {
        Log.i(TAG, "client subscribed to CTRL_TX")
    }

    override fun onDisconnected() {
        Log.i(TAG, "client disconnected; queue will flush on reconnect")
    }

    private fun sendControl(plaintext: ByteArray, handshake: Boolean) {
        val id = nextMessageId++
        val flagged = if (handshake) id or GattServerManager.HANDSHAKE_BIT else id
        gattServer.sendControlMessage(flagged, plaintext, gattServer.currentMtu)
    }

    // --- Notification --------------------------------------------------

    private fun buildNotification(): Notification {
        val manager = getSystemService(NotificationManager::class.java)
        if (Build.VERSION.SDK_INT >= 26) {
            manager.createNotificationChannel(
                NotificationChannel(CHANNEL_ID, getString(app.pocketbridge.bridge.R.string.channel_name), NotificationManager.IMPORTANCE_LOW)
            )
        }
        val intent = PendingIntent.getActivity(
            this, 0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE,
        )
        return Notification.Builder(this, CHANNEL_ID)
            .setContentTitle(getString(app.pocketbridge.bridge.R.string.notif_title))
            .setContentText(getString(app.pocketbridge.bridge.R.string.notif_text))
            .setSmallIcon(android.R.drawable.stat_sys_data_bluetooth)
            .setContentIntent(intent)
            .setOngoing(true)
            .build()
    }

    private fun serviceTypes(): Int =
        if (Build.VERSION.SDK_INT >= 29) {
            ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE or
                ServiceInfo.FOREGROUND_SERVICE_TYPE_PHONE_CALL
        } else {
            0
        }

    companion object {
        private const val TAG = "PBBridgeService"
        private const val CHANNEL_ID = "pocketbridge_bridge"
        private const val NOTIFICATION_ID = 42

        fun start(context: Context) {
            context.startForegroundService(Intent(context, BridgeForegroundService::class.java))
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, BridgeForegroundService::class.java))
        }
    }
}
