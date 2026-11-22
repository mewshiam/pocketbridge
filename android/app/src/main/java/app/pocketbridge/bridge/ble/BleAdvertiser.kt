package app.pocketbridge.bridge.ble

import android.annotation.SuppressLint
import android.bluetooth.le.AdvertiseCallback
import android.bluetooth.le.AdvertiseData
import android.bluetooth.le.AdvertiseSettings
import android.bluetooth.le.BluetoothLeAdvertiser
import android.content.Context
import android.os.ParcelUuid
import android.util.Log
import java.security.SecureRandom

/**
 * BleAdvertiser — advertises the PocketBridge primary service UUID with an
 * ephemeral 8-hex-char short ID in Manufacturer Specific Data so the iOS
 * client can disambiguate nearby bridges (the QR's `adv` field).
 */
@SuppressLint("MissingPermission")
class BleAdvertiser(context: Context) {

    private val adapter = (context.getSystemService(Context.BLUETOOTH_SERVICE) as? android.bluetooth.BluetoothManager)
        ?.adapter

    /** Ephemeral advertiser short ID, regenerated per pairing session. */
    val advId: String = generateAdvId()

    private var advertising = false

    fun start(): Boolean {
        val advertiser: BluetoothLeAdvertiser = adapter?.bluetoothLeAdvertiser ?: return false
        if (advertising) return true

        val settings = AdvertiseSettings.Builder()
            .setAdvertiseMode(AdvertiseSettings.ADVERTISE_MODE_LOW_LATENCY)
            .setTxPowerLevel(AdvertiseSettings.ADVERTISE_TX_POWER_MEDIUM)
            .setConnectable(true)
            .build()

        val data = AdvertiseData.Builder()
            .setIncludeDeviceName(false)
            .addManufacturerData(MANUFACTURER_ID, advId.encodeToByteArray())
            .addServiceUuid(ParcelUuid(GattServerManager.SERVICE_UUID))
            .build()

        advertiser.startAdvertising(settings, data, callback)
        advertising = true
        return true
    }

    fun stop() {
        val advertiser: BluetoothLeAdvertiser = adapter?.bluetoothLeAdvertiser ?: return
        if (advertising) {
            advertiser.stopAdvertising(callback)
            advertising = false
        }
    }

    private val callback = object : AdvertiseCallback() {
        override fun onStartSuccess(settingsInEffect: AdvertiseSettings) {
            Log.i(TAG, "advertising started as $advId")
        }

        override fun onStartFailure(errorCode: Int) {
            Log.w(TAG, "advertising failed: $errorCode")
        }
    }

    companion object {
        private const val TAG = "PBBleAdvertiser"

        /** Registered manufacturer ID for PocketBridge (random, non-official). */
        const val MANUFACTURER_ID = 0xFF57

        /** 8 hex chars from a CSPRNG (4 bytes -> 8 hex). */
        fun generateAdvId(): String {
            val bytes = ByteArray(4)
            SecureRandom().nextBytes(bytes)
            return bytes.joinToString("") { "%02X".format(it) }
        }
    }
}
