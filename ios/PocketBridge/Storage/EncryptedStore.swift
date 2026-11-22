import Foundation

/// EncryptedStore — SQLCipher-backed local store for synced SMS, contacts,
/// and call logs (blueprint Task 5). The protocol is the contract; tests use
/// the in-memory implementation.
public protocol EncryptedStoreProtocol {
    func saveSms(address: String, body: String, timestampMs: Int64) throws
    func smsHistory(limit: Int) -> [StoredSms]
    func smsCount() -> Int
}

public struct StoredSms: Equatable {
    public let address: String
    public let body: String
    public let timestampMs: Int64
    public init(address: String, body: String, timestampMs: Int64) {
        self.address = address; self.body = body; self.timestampMs = timestampMs
    }
}

/// In-memory reference implementation (test target).
public final class InMemoryEncryptedStore: EncryptedStoreProtocol {
    private var messages: [StoredSms] = []
    public init() {}
    public func saveSms(address: String, body: String, timestampMs: Int64) {
        messages.append(StoredSms(address: address, body: body, timestampMs: timestampMs))
    }
    public func smsHistory(limit: Int) -> [StoredSms] { messages.suffix(limit) }
    public func smsCount() -> Int { messages.count }
}
