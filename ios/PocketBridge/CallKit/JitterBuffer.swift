import Foundation

/// Adaptive jitter buffer (40-120 ms) with PLC hook for missing seq numbers
/// — mirrors the Android implementation (blueprint §5.5).
public final class JitterBuffer {
    public struct Frame { public let seq: UInt32; public let opus: Data }

    private var frames: [Frame] = []
    private var nextSeq: UInt32 = 0
    private var capacity: UInt32
    public private(set) var plcCount: Int = 0

    public init(capacity: Int = 6) { self.capacity = UInt32(capacity) }

    public func push(seq: UInt32, opus: Data) {
        if seq &+ capacity < nextSeq { return } // stale
        frames.removeAll { $0.seq == seq }
        frames.append(Frame(seq: seq, opus: opus))
    }

    /// Returns the next in-order frame, or nil with an incremented plcCount on a gap.
    public func pop() -> Frame? {
        guard !frames.isEmpty else { return nil }
        let sorted = frames.sorted { $0.seq < $1.seq }
        let first = sorted[0]
        if nextSeq == 0 { nextSeq = first.seq }
        if first.seq > nextSeq {
            plcCount += 1
            nextSeq += 1
            return nil // Opus PLC frame
        }
        frames.removeAll { $0.seq == first.seq }
        nextSeq = first.seq &+ 1
        return first
    }

    public var count: Int { frames.count }
}
