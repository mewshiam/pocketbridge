import XCTest
@testable import PocketBridge

final class EnvelopeRouterTests: XCTestCase {
    func testPingProducesPong() {
        let reply = EnvelopeRouter.dispatch(Data("ping:42".utf8), handlers: EnvelopeRouter.Handlers())
        XCTAssertEqual(String(data: reply ?? Data(), encoding: .utf8), "pong:42")
    }

    func testPongDispatchesHandler() {
        var got: UInt32?
        var handlers = EnvelopeRouter.Handlers()
        handlers.onPong = { got = $0 }
        _ = EnvelopeRouter.dispatch(Data("pong:7".utf8), handlers: handlers)
        XCTAssertEqual(got, 7)
    }

    func testDeviceStatusDispatches() {
        var got: String?
        var handlers = EnvelopeRouter.Handlers()
        handlers.onDeviceStatus = { got = $0 }
        _ = EnvelopeRouter.dispatch(Data("status:battery=80".utf8), handlers: handlers)
        XCTAssertEqual(got, "battery=80")
    }

    func testIncomingSmsParsesAddressAndBody() {
        var address: String?
        var body: String?
        var handlers = EnvelopeRouter.Handlers()
        handlers.onSmsReceived = { address = $0; body = $1 }
        _ = EnvelopeRouter.dispatch(Data("smsin:+1555|hello".utf8), handlers: handlers)
        XCTAssertEqual(address, "+1555")
        XCTAssertEqual(body, "hello")
    }

    func testCallStateDispatches() {
        var got: String?
        var handlers = EnvelopeRouter.Handlers()
        handlers.onCallState = { got = $0 }
        _ = EnvelopeRouter.dispatch(Data("call:ACTIVE".utf8), handlers: handlers)
        XCTAssertEqual(got, "ACTIVE")
    }
}
