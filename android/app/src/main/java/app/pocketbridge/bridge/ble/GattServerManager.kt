package app.pocketbridge.bridge.ble

import android.annotation.SuppressLint
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothGattServer
import android.bluetooth.BluetoothGattServerCallback
import android.bluetooth.BluetoothManager
import android.content.Context
import android.util.Log
import java.util.UUID

/**
 * GattServerManager — the Bridge side of the PocketBridge GATT service.
 *
 * Service layout (protocol/SPEC.md §2):
 *  - CTRL_TX   (notify)            : encrypted control frames, Bridge -> Client
 *  - CTRL_RX   (write, writeNR)    : encrypted control frames, Client -> Bridge
 *  - L2CAP_PSM (read)              : little-endian uint16 dynamic PSM
 *  - DEVICE_INFO (read)            : [proto_major, proto_minor, feature_flags]
 */
@SuppressLint("MissingPermission")
class GattServerManager(private val context: Context) {

    interface Listener {
        /** A complete (handshake or transport) message arrived on CTRL_RX. */
        fun onControlMessage(bytes: ByteArray)

        /** Peer subscribed to CTRL_TX notifications. */
        fun onSubscribed()

        /** Peer disconnected. */
        fun onDisconnected()
    }

    var listener: Listener? = null

    private val bluetoothManager: BluetoothManager? =
        context.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager

    private var gattServer: BluetoothGattServer? = null

    /** Current dynamic PSM for the voice L2CAP channel (little-endian on wire). */
    @Volatile
    var l2capPsm: Int = DEFAULT_PSM

    /** Feature flags advertised in DEVICE_INFO (see SPEC.md §2). */
    var featureFlags: Int =
        FLAG_L2CAP_VOICE or FLAG_DUAL_SIM or FLAG_INCREMENTAL_CONTACTS or FLAG_OPUS_FEC_DTX

    /** Handler invoked when the reassembler emits a complete message. */
    private val reassembler = uniffi.pocketbridge_core.ControlReassembler()

    fun start(): Boolean {
        val manager = bluetoothManager ?: return false
        val server = try {
            manager.openGattServer(context, callback)
        } catch (e: Exception) {
            Log.w(TAG, "openGattServer failed: ${e.message}")
            return false
        }
        gattServer = server ?: return false

        val service = android.bluetooth.BluetoothGattService(SERVICE_UUID, android.bluetooth.BluetoothGattService.SERVICE_TYPE_PRIMARY)

        val ctrlTx = BluetoothGattCharacteristic(
            CTRL_TX_UUID,
            BluetoothGattCharacteristic.PROPERTY_NOTIFY,
            BluetoothGattCharacteristic.PERMISSION_READ
        )
        val cccd = BluetoothGattDescriptor(
            CCCD_UUID,
            android.bluetooth.BluetoothGattDescriptor.PERMISSION_READ or
                android.bluetooth.BluetoothGattDescriptor.PERMISSION_WRITE
        )
        cccd.value = android.bluetooth.BluetoothGattDescriptor.DISABLE_NOTIFICATION_VALUE
        ctrlTx.addDescriptor(cccd)

        val ctrlRx = BluetoothGattCharacteristic(
            CTRL_RX_UUID,
            BluetoothGattCharacteristic.PROPERTY_WRITE or 0x04, // PROPERTY_WRITE_WITHOUT_RESPONSE
            BluetoothGattCharacteristic.PERMISSION_WRITE
        )

        val psm = BluetoothGattCharacteristic(
            L2CAP_PSM_UUID,
            BluetoothGattCharacteristic.PROPERTY_READ,
            BluetoothGattCharacteristic.PROPERTY_READ
        )

        val deviceInfo = BluetoothGattCharacteristic(
            DEVICE_INFO_UUID,
            BluetoothGattCharacteristic.PROPERTY_READ,
            BluetoothGattCharacteristic.PROPERTY_READ
        )

        service.addCharacteristic(ctrlTx)
        service.addCharacteristic(ctrlRx)
        service.addCharacteristic(psm)
        service.addCharacteristic(deviceInfo)

        return gattServer?.addService(service) ?: false
    }

    fun stop() {
        gattServer?.close()
        gattServer = null
    }

    /** Sends one (already-encrypted) message via CTRL_TX in MTU-sized chunks. */
    fun sendControlMessage(messageId: Int, encrypted: ByteArray, mtu: Int = DEFAULT_MTU): Boolean {
        val server = gattServer ?: return false
        val tx = server.getService(SERVICE_UUID)?.getCharacteristic(CTRL_TX_UUID) ?: return false
        // Handshake traffic (flag 0x04) is single-chunk; transport traffic
        // uses START/END framing. The caller flags handshake via bit set in
        // the message id's top bit — see BridgeForegroundService.
        val isHandshake = messageId and HANDSHAKE_BIT != 0
        val chunks = uniffi.pocketbridge_core.chunkControlMessage(
            messageId = (messageId and 0x7FFF).toUShort(),
            data = encrypted,
            mtu = mtu.toUInt(),
            handshake = isHandshake,
        )
        for (chunk in chunks) {
            tx.value = chunk
            val ok = server.notifyCharacteristicChanged(pendingConnection(), tx, false)
            if (!ok) {
                Log.w(TAG, "notifyCharacteristicChanged failed")
                return false
            }
        }
        return true
    }

    private fun pendingConnection(): android.bluetooth.BluetoothDevice? = pendingDevice

    @Volatile
    private var pendingDevice: android.bluetooth.BluetoothDevice? = null

    private val callback = object : BluetoothGattServerCallback() {
        override fun onCharacteristicWriteRequest(
            device: android.bluetooth.BluetoothDevice,
            requestId: Int,
            characteristic: BluetoothGattCharacteristic,
            preparedWrite: Boolean,
            responseNeeded: Boolean,
            offset: Int,
            value: ByteArray,
        ) {
            val server = gattServer ?: return
            if (responseNeeded) {
                server.sendResponse(device, requestId, android.bluetooth.BluetoothGatt.GATT_SUCCESS, 0, value)
            }
            if (characteristic.uuid == CTRL_RX_UUID) {
                pendingDevice = device
                // Handshake chunks bypass START/END; transport chunks stream
                // through the reassembler. Control both paths through the
                // same Rust core.
                val header = runCatching { uniffi.pocketbridge_core.readChunkHeader(value) }.getOrNull()
                val isHandshakeChunk = header != null && (header.flags.toInt() and FLAG_HANDSHAKE) == FLAG_HANDSHAKE
                val message = if (isHandshakeChunk) {
                    value.copyOfRange(HEADER_LEN, value.size)
                } else {
                    reassembler.feed(value) ?: return
                }
                listener?.onControlMessage(message)
            }
        }

        override fun onDescriptorWriteRequest(
            device: android.bluetooth.BluetoothDevice,
            requestId: Int,
            descriptor: BluetoothGattDescriptor,
            preparedWrite: Boolean,
            responseNeeded: Boolean,
            offset: Int,
            value: ByteArray,
        ) {
            val server = gattServer ?: return
            if (responseNeeded) {
                server.sendResponse(device, requestId, android.bluetooth.BluetoothGatt.GATT_SUCCESS, 0, value)
            }
            if (descriptor.uuid == CCCD_UUID &&
                value.contentEquals(android.bluetooth.BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE)
            ) {
                listener?.onSubscribed()
            }
        }

        override fun onCharacteristicReadRequest(
            device: android.bluetooth.BluetoothDevice,
            requestId: Int,
            offset: Int,
            characteristic: BluetoothGattCharacteristic,
        ) {
            val server = gattServer ?: return
            val payload: ByteArray = when (characteristic.uuid) {
                L2CAP_PSM_UUID -> byteArrayOf(
                    (l2capPsm and 0xFF).toByte(),
                    ((l2capPsm shr 8) and 0xFF).toByte(),
                )
                DEVICE_INFO_UUID -> byteArrayOf(
                    PROTOCOL_MAJOR.toByte(),
                    PROTOCOL_MINOR.toByte(),
                    (featureFlags and 0xFF).toByte(),
                    ((featureFlags shr 8) and 0xFF).toByte(),
                )
                else -> ByteArray(0)
            }
            server.sendResponse(
                device,
                requestId,
                android.bluetooth.BluetoothGatt.GATT_SUCCESS,
                offset,
                payload,
            )
        }

        override fun onConnectionStateChange(
            device: android.bluetooth.BluetoothDevice,
            status: Int,
            newState: Int,
        ) {
            if (newState == android.bluetooth.BluetoothProfile.STATE_DISCONNECTED) {
                pendingDevice = null
                listener?.onDisconnected()
            }
        }

        override fun onMtuChanged(device: android.bluetooth.BluetoothDevice, mtu: Int) {
            currentMtu = mtu
        }
    }

    @Volatile
    var currentMtu: Int = DEFAULT_MTU

    companion object {
        private const val TAG = "PBGattServer"

        val SERVICE_UUID: UUID = UUID.fromString("8f2b0001-6a4e-4c1d-9b3a-1e5f7d2c8a01")
        val CTRL_TX_UUID: UUID = UUID.fromString("8f2b0002-6a4e-4c1d-9b3a-1e5f7d2c8a01")
        val CTRL_RX_UUID: UUID = UUID.fromString("8f2b0003-6a4e-4c1d-9b3a-1e5f7d2c8a01")
        val L2CAP_PSM_UUID: UUID = UUID.fromString("8f2b0004-6a4e-4c1d-9b3a-1e5f7d2c8a01")
        val DEVICE_INFO_UUID: UUID = UUID.fromString("8f2b0005-6a4e-4c1d-9b3a-1e5f7d2c8a01")
        private val CCCD_UUID: UUID = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")

        const val PROTOCOL_MAJOR = 1
        const val PROTOCOL_MINOR = 0
        const val DEFAULT_PSM = 0x0057
        const val DEFAULT_MTU = 517

        const val FLAG_HANDSHAKE = 0x04
        const val HANDSHAKE_BIT = 0x8000
        const val HEADER_LEN = 6

        const val FLAG_L2CAP_VOICE = 0x0001
        const val FLAG_DUAL_SIM = 0x0002
        const val FLAG_INCREMENTAL_CONTACTS = 0x0004
        const val FLAG_OPUS_FEC_DTX = 0x0008
    }
}
