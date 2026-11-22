import Foundation
import CoreBluetooth

/// BluetoothCentralManager — the iOS client side of the PocketBridge GATT link.
///
/// Uses `CBCentralManagerOptionRestoreIdentifierKey` for state restoration
/// so background wake works (blueprint §7.3). Scans for the primary service
/// UUID, connects, subscribes to CTRL_TX, writes to CTRL_RX, and surfaces
/// device-info + PSM reads.
public final class BluetoothCentralManager: NSObject, CBCentralManagerDelegate {

    public static let serviceUUID = CBUUID(string: "8F2B0001-6A4E-4C1D-9B3A-1E5F7D2C8A01")
    public static let ctrlTxUUID = CBUUID(string: "8F2B0002-6A4E-4C1D-9B3A-1E5F7D2C8A01")
    public static let ctrlRxUUID = CBUUID(string: "8F2B0003-6A4E-4C1D-9B3A-1E5F7D2C8A01")
    public static let l2capPsmUUID = CBUUID(string: "8F2B0004-6A4E-4C1D-9B3A-1E5F7D2C8A01")
    public static let deviceInfoUUID = CBUUID(string: "8F2B0005-6A4E-4C1D-9B3A-1E5F7D2C8A01")
    public static let restoreIdentifier = "app.pocketbridge.central"

    /// Connection state surfaced to SwiftUI.
    public enum State: Equatable {
        case idle, scanning, connecting, connected, disconnected(String)
    }

    public struct DeviceInfo: Equatable {
        public let protoMajor: UInt8
        public let protoMinor: UInt8
        public let featureFlags: UInt16
        public var supportsVoice: Bool { featureFlags & 0x0001 != 0 }
        public var supportsDualSim: Bool { featureFlags & 0x0002 != 0 }
        public var supportsContactSync: Bool { featureFlags & 0x0004 != 0 }
        public var supportsOpusFecDtx: Bool { featureFlags & 0x0008 != 0 }

        /// Parses the raw DEVICE_INFO bytes [major, minor, flags LE].
        public init?(bytes: Data) {
            guard bytes.count >= 4 else { return nil }
            protoMajor = bytes[bytes.startIndex]
            protoMinor = bytes[bytes.index(bytes.startIndex, offsetBy: 1)]
            let lo = UInt16(bytes[bytes.index(bytes.startIndex, offsetBy: 2)])
            let hi = UInt16(bytes[bytes.index(bytes.startIndex, offsetBy: 3)])
            featureFlags = lo | (hi << 8)
        }
    }

    public var stateHandler: ((State) -> Void)?
    public var controlMessageHandler: ((Data) -> Void)?
    public var deviceInfoHandler: ((DeviceInfo) -> Void)?

    private var central: CBCentralManager?
    private var peripheral: CBPeripheral?
    private var ctrlTx: CBCharacteristic?

    /// Scans and connects to the advertised bridge.
    public func start() {
        if central == nil {
            central = CBCentralManager(
                delegate: self,
                queue: nil,
                options: [CBCentralManagerOptionRestoreIdentifierKey: Self.restoreIdentifier]
            )
        } else {
            scan()
        }
    }

    private func scan() {
        stateHandler?(.scanning)
        central?.scanForPeripherals(
            withServices: [Self.serviceUUID],
            options: [CBCentralManagerScanOptionAllowDuplicatesKey: false]
        )
    }

    public func stop() {
        central?.stopScan()
        if let peripheral { central?.cancelPeripheralConnection(peripheral) }
        peripheral = nil
    }

    /// Writes one control-plane message on CTRL_RX.
    public func sendControl(_ data: Data) {
        guard let peripheral, let characteristic = peripheral
            .service(withUUID: Self.serviceUUID)?
            .characteristic(withUUID: Self.ctrlRxUUID) else { return }
        peripheral.writeValue(data, for: characteristic, type: .withoutResponse)
    }

    // MARK: - CBCentralManagerDelegate

    public func centralManagerDidUpdateState(_ central: CBCentralManager) {
        switch central.state {
        case .poweredOn: scan()
        default: stateHandler?(.idle)
        }
    }

    public func centralManager(
        _ central: CBCentralManager,
        didDiscover peripheral: CBPeripheral,
        advertisementData: [String: Any],
        rssi RSSI: NSNumber
    ) {
        self.peripheral = peripheral
        peripheral.delegate = self
        stateHandler?(.connecting)
        central.stopScan()
        central.connect(peripheral, options: nil)
    }

    public func centralManager(
        _ central: CBCentralManager,
        didConnect peripheral: CBPeripheral
    ) {
        peripheral.discoverServices([Self.serviceUUID])
    }

    public func centralManager(
        _ central: CBCentralManager,
        didDisconnectPeripheral peripheral: CBPeripheral,
        error: Error?
    ) {
        stateHandler?(.disconnected(error?.localizedDescription ?? "peer left range"))
        // Background reconnect loop: keep trying while the app lives.
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { [weak self] in
            self?.scan()
        }
    }

    /// State restoration wake (background modes: bluetooth-central).
    public func centralManager(
        _ central: CBCentralManager,
        willRestoreState dict: [String: Any]
    ) {
        if let peripherals = dict[CBCentralManagerRestoredStatePeripheralsKey] as? [CBPeripheral] {
            peripheral = peripherals.first
            peripheral?.delegate = self
        }
    }
}

extension BluetoothCentralManager: CBPeripheralDelegate {
    public func peripheral(
        _ peripheral: CBPeripheral,
        didDiscoverServices error: Error?
    ) {
        peripheral.discoverCharacteristics(
            [Self.ctrlTxUUID, Self.ctrlRxUUID, Self.l2capPsmUUID, Self.deviceInfoUUID],
            for: peripheral.service(withUUID: Self.serviceUUID) ?? CBMutableService(type: Self.serviceUUID, primary: true)
        )
    }

    public func peripheral(
        _ peripheral: CBPeripheral,
        didDiscoverCharacteristicsFor service: CBService,
        error: Error?
    ) {
        for characteristic in service.characteristics ?? [] where characteristic.uuid == Self.ctrlTxUUID {
            ctrlTx = characteristic
            peripheral.setNotifyValue(true, for: characteristic)
        }
        if let infoCharacteristic = service.characteristics?
            .first(where: { $0.uuid == Self.deviceInfoUUID }) {
            peripheral.readValue(for: infoCharacteristic)
        }
        if let psmCharacteristic = service.characteristics?
            .first(where: { $0.uuid == Self.l2capPsmUUID }) {
            peripheral.readValue(for: psmCharacteristic)
        }
        stateHandler?(.connected)
    }

    public func peripheral(
        _ peripheral: CBPeripheral,
        didUpdateValueFor characteristic: CBCharacteristic,
        error: Error?
    ) {
        guard let data = characteristic.value else { return }
        switch characteristic.uuid {
        case Self.ctrlTxUUID: controlMessageHandler?(data)
        case Self.deviceInfoUUID:
            if let info = DeviceInfo(bytes: data) { deviceInfoHandler?(info) }
        default: break
        }
    }
}
