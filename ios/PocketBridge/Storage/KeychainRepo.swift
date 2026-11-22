import Foundation
import Security

/// KeychainRepo — static pairing keys live exclusively in the iOS Keychain
/// with `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly` (blueprint §1.3).
///
/// The test target injects an in-memory backing so Keychain semantics are
/// verifiable without a simulator Keychain.
public final class KeychainRepo {

    /// Abstraction over SecItemAdd/SecItemCopyMatching for tests.
    public protocol Backing {
        func set(_ data: Data, forKey key: String)
        func data(forKey key: String) -> Data?
        func removeValue(forKey key: String)
    }

    /// Real Keychain backing (device + simulator).
    public struct KeychainBacking: Backing {
        public init() {}

        private let service = "app.pocketbridge.client"
        private let accessibility = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly

        public func set(_ data: Data, forKey key: String) {
            removeValue(forKey: key)
            let query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: service,
                kSecAttrAccount as String: key,
                kSecValueData as String: data,
                kSecAttrAccessible as String: accessibility,
            ]
            SecItemAdd(query as CFDictionary, nil)
        }

        public func data(forKey key: String) -> Data? {
            let query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: service,
                kSecAttrAccount as String: key,
                kSecReturnData as String: true,
                kSecMatchLimit as String: kSecMatchLimitOne,
            ]
            var result: AnyObject?
            let status = SecItemCopyMatching(query as CFDictionary, &result)
            guard status == errSecSuccess else { return nil }
            return result as? Data
        }

        public func removeValue(forKey key: String) {
            let query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: service,
                kSecAttrAccount as String: key,
            ]
            SecItemDelete(query as CFDictionary)
        }
    }

    /// In-memory backing for unit tests.
    public final class MemoryBacking: Backing {
        private var store: [String: Data] = [:]
        public init() {}
        public func set(_ data: Data, forKey key: String) { store[key] = data }
        public func data(forKey key: String) -> Data? { store[key] }
        public func removeValue(forKey key: String) { store.removeValue(forKey: key) }
    }

    private let backing: Backing

    public init(backing: Backing = KeychainBacking()) {
        self.backing = backing
    }

    /// Pinned bridge static key (from the completed Noise_XX handshake).
    public func savePinnedBridgeKey(_ key: Data) { backing.set(key, forKey: Keys.pinnedBridge) }
    public func loadPinnedBridgeKey() -> Data? { backing.data(forKey: Keys.pinnedBridge) }

    /// Our static identity keypair.
    public func saveLocalPrivateKey(_ key: Data) { backing.set(key, forKey: Keys.localPrivate) }
    public func loadLocalPrivateKey() -> Data? { backing.data(forKey: Keys.localPrivate) }
    public func saveLocalPublicKey(_ key: Data) { backing.set(key, forKey: Keys.localPublic) }
    public func loadLocalPublicKey() -> Data? { backing.data(forKey: Keys.localPublic) }

    public func clearPairing() {
        backing.removeValue(forKey: Keys.pinnedBridge)
        backing.removeValue(forKey: Keys.localPrivate)
        backing.removeValue(forKey: Keys.localPublic)
    }

    public func isPaired() -> Bool { backing.data(forKey: Keys.pinnedBridge) != nil }

    private enum Keys {
        static let pinnedBridge = "pinned_bridge_key"
        static let localPrivate = "local_private_key"
        static let localPublic = "local_public_key"
    }
}
