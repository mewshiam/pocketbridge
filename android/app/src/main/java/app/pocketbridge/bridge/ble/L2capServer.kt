package app.pocketbridge.bridge.ble

import android.annotation.SuppressLint
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothServerSocket
import android.bluetooth.BluetoothSocket
import android.content.Context
import android.util.Log
import java.io.InputStream
import java.io.OutputStream

/**
 * L2capServer — voice-channel listener on a dynamic PSM.
 *
 * Opened only when a call enters RINGING/ACTIVE (blueprint §5.5). Frames are
 * AEAD-protected by the Rust core (audio_crypto); L2CAP security is enforced
 * at our encryption layer, so the insecure channel is intentional.
 */
@SuppressLint("MissingPermission")
class L2capServer(private val context: Context) {

    private var serverSocket: BluetoothServerSocket? = null
    @Volatile
    var psm: Int = 0
        private set

    /** Opens the channel and reports the dynamic PSM (for the L2CAP_PSM characteristic). */
    fun listen(adapter: BluetoothAdapter?): Boolean {
        val a = adapter ?: (context.getSystemService(Context.BLUETOOTH_SERVICE) as? android.bluetooth.BluetoothManager)?.adapter
        ?: return false
        return try {
            serverSocket = a.listenUsingInsecureL2capChannel()
            psm = serverSocket?.psm ?: 0
            Log.i(TAG, "L2CAP listening on PSM $psm")
            true
        } catch (e: Exception) {
            Log.w(TAG, "L2CAP listen failed: ${e.message}")
            false
        }
    }

    /** Blocks for one client; returns its streams. */
    fun accept(): Pair<InputStream, OutputStream>? {
        val socket: BluetoothSocket = serverSocket?.accept() ?: return null
        return socket.inputStream to socket.outputStream
    }

    fun close() {
        serverSocket?.close()
        serverSocket = null
    }

    companion object { private const val TAG = "PBL2capServer" }
}
