package app.pocketbridge.bridge.audio

/**
 * Adaptive jitter buffer for L2CAP voice (blueprint §5.5): 40-120 ms window,
 * 20 ms Opus frames, packet-loss concealment hook on missing sequence numbers.
 * Pure logic — unit-testable without Android.
 */
class JitterBuffer(private val capacity: Int = 6) {

    data class Frame(val seq: UInt, val opus: ByteArray)

    private val frames = mutableListOf<Frame>()
    private var nextSeq: UInt = 0u
    var plcCount = 0
        private set

    /** Adds an out-of-order frame; drops stale ones past the window. */
    fun push(seq: UInt, opus: ByteArray) {
        if (seq + capacity.toUInt() < nextSeq) return // too old
        frames.removeAll { it.seq == seq }
        frames.add(Frame(seq, opus))
    }

    /** Pops the next frame in order; invokes PLC on gaps (missing seq). */
    fun pop(): Frame? {
        if (frames.isEmpty()) return null
        val sorted = frames.sortedBy { it.seq }
        val first = sorted.first()
        if (nextSeq == 0u && !frames.any { it.seq < first.seq }) nextSeq = first.seq
        if (first.seq > nextSeq) {
            // Gap: conceal one frame, advance, retry on next pop.
            plcCount++
            nextSeq++
            return null // caller feeds Opus PLC
        }
        frames.remove(first)
        nextSeq = first.seq + 1u
        return first
    }

    fun size() = frames.size
}
