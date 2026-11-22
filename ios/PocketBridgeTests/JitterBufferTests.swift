import XCTest
@testable import PocketBridge

final class JitterBufferTests: XCTestCase {
    func testInOrder() {
        let jb = JitterBuffer()
        jb.push(seq: 0, opus: Data([1])); jb.push(seq: 1, opus: Data([2]))
        XCTAssertEqual(jb.pop()?.seq, 0); XCTAssertEqual(jb.pop()?.seq, 1)
    }
    func testReorder() {
        let jb = JitterBuffer()
        jb.push(seq: 2, opus: Data([3])); jb.push(seq: 0, opus: Data([1])); jb.push(seq: 1, opus: Data([2))
        XCTAssertEqual(jb.pop()?.seq, 0); XCTAssertEqual(jb.pop()?.seq, 1); XCTAssertEqual(jb.pop()?.seq, 2)
    }
    func testGapTriggersPlc() {
        let jb = JitterBuffer()
        jb.push(seq: 0, opus: Data([1])); jb.push(seq: 2, opus: Data([3))
        _ = jb.pop()
        XCTAssertNil(jb.pop())
        XCTAssertEqual(jb.plcCount, 1)
        XCTAssertEqual(jb.pop()?.seq, 2)
    }
}
