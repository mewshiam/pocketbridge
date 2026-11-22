import Foundation
import CallKit
import AVFoundation

/// CallKitManager — native lock-screen ringing, AirPods/CarPlay routing
/// (blueprint §7.3). Bridges CXProviderDelegate state transitions into
/// CallStateEvent flow and CallActionRequests back to the bridge.
public final class CallKitManager: NSObject, CXProviderDelegate {

    public static let shared = CallKitManager()

    private var provider: CXProvider?
    private let controller = CXCallController()

    public var onAnswer: (() -> Void)?
    public var onEnd: (() -> Void)?

    public func configure() {
        let config = CXProviderConfiguration(localizedName: "PocketBridge")
        config.supportsVideo = false
        config.supportedHandleTypes = [.phoneNumber]
        provider = CXProvider(configuration: config)
        provider?.setDelegate(self, queue: nil)
    }

    /// Lock-screen incoming call UI.
    public func reportIncomingCall(handle: String, completion: ((Error?) -> Void)? = nil) {
        let update = CXCallUpdate()
        update.remoteHandle = CXHandle(type: .phoneNumber, value: handle)
        update.supportsHolding = true
        provider?.reportNewIncomingCall(with: update) { error in
            completion?(error)
        }
    }

    public func providerDidActivateAudioSession(_ provider: CXProvider) {
        onAnswer?()
    }

    public func provider(_ provider: CXProvider, perform action: CXAnswerCallAction) {
        action.fulfill()
        onAnswer?()
    }

    public func provider(_ provider: CXProvider, perform action: CXEndCallAction) {
        action.fulfill()
        onEnd?()
    }
}
