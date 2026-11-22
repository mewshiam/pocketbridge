import XCTest
@testable import PocketBridge

final class KeychainRepoTests: XCTestCase {
    func testRoundTripPinnedKey() {
        let repo = KeychainRepo(backing: KeychainRepo.MemoryBacking())
        let key = Data((0..<32).map { UInt8($0) })
        repo.savePinnedBridgeKey(key)
        XCTAssertEqual(repo.loadPinnedBridgeKey(), key)
        XCTAssertTrue(repo.isPaired())
    }

    func testClearRemovesPairing() {
        let repo = KeychainRepo(backing: KeychainRepo.MemoryBacking())
        repo.savePinnedBridgeKey(Data([0x01]))
        repo.clearPairing()
        XCTAssertFalse(repo.isPaired())
        XCTAssertNil(repo.loadPinnedBridgeKey())
    }

    func testLocalKeysSurviveRoundTrip() {
        let repo = KeychainRepo(backing: KeychainRepo.MemoryBacking())
        let priv = Data(repeating: 0x07, count: 32)
        repo.saveLocalPrivateKey(priv)
        XCTAssertEqual(repo.loadLocalPrivateKey(), priv)
    }
}
