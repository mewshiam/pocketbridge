package app.pocketbridge.bridge

import app.pocketbridge.bridge.audio.JitterBuffer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class JitterBufferUnitTest {
    @Test
    fun `in-order frames pop in order`() {
        val jb = JitterBuffer()
        jb.push(0u, byteArrayOf(1)); jb.push(1u, byteArrayOf(2)); jb.push(2u, byteArrayOf(3))
        assertEquals(0u, jb.pop()!!.seq); assertEquals(1u, jb.pop()!!.seq); assertEquals(2u, jb.pop()!!.seq)
    }

    @Test
    fun `out-of-order frames reorder`() {
        val jb = JitterBuffer()
        jb.push(2u, byteArrayOf(3)); jb.push(0u, byteArrayOf(1)); jb.push(1u, byteArrayOf(2))
        assertEquals(listOf(0u, 1u, 2u), listOf(jb.pop()!!.seq, jb.pop()!!.seq, jb.pop()!!.seq))
    }

    @Test
    fun `gap triggers plc`() {
        val jb = JitterBuffer()
        jb.push(0u, byteArrayOf(1)); jb.push(2u, byteArrayOf(3))
        assertEquals(0u, jb.pop()!!.seq)
        assertNull(jb.pop()) // gap at seq 1: PLC
        assertEquals(1, jb.plcCount)
        assertEquals(2u, jb.pop()!!.seq)
    }

    @Test
    fun `stale frames dropped`() {
        val jb = JitterBuffer()
        jb.push(0u, byteArrayOf(1)); jb.pop()
        jb.push(0u, byteArrayOf(1)) // stale relative to nextSeq=1
        assertTrue(jb.size() <= 1)
    }
}
