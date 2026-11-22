import Foundation

/// Envelope dispatch on iOS: routes decrypted control-plane traffic to
/// heartbeat, SMS, call, and sync handlers (blueprint Task 4/5).
///
/// Pure logic — fully unit-testable without CoreBluetooth.
public enum EnvelopeRouter {

    public struct Handlers {
        public var onPong: ((UInt32) -> Void)?
        public var onDeviceStatus: ((String) -> Void)?
        public var onSmsReceived: ((_ address: String, _ body: String) -> Void)?
        public var onCallState: ((String) -> Void)?
        public init() {}
    }

    /// Parses a decrypted control message and dispatches to handlers.
    /// Returns the reply bytes to send back, if any (e.g. Ping -> Pong).
    public static func dispatch(_ data: Data, handlers: Handlers) -> Data? {
        guard let text = String(data: data, encoding: .utf8) else { return nil }
        if let seqString = text.split(separator: ":", maxSplits: 1).last,
           text.hasPrefix("ping:") {
            let seq = UInt32(seqString) ?? 0
            return Data("pong:\(seq)".utf8)
        }
        if text.hasPrefix("pong:") {
            let seq = UInt32(text.dropFirst(5)) ?? 0
            handlers.onPong?(seq)
            return nil
        }
        if text.hasPrefix("status:") {
            handlers.onDeviceStatus?(String(text.dropFirst(7)))
            return nil
        }
        if text.hasPrefix("smsin:") {
            // "smsin:<address>|<body>"
            let rest = text.dropFirst(6).split(separator: "|", maxSplits: 1)
            if rest.count == 2 {
                handlers.onSmsReceived?(String(rest[0]), String(rest[1]))
            }
            return nil
        }
        if text.hasPrefix("call:") {
            handlers.onCallState?(String(text.dropFirst(5)))
            return nil
        }
        return nil
    }
}
