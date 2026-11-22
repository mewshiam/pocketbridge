import SwiftUI

@main
public struct PocketBridgeApp: App {
    public init() {}
    public var body: some Scene {
        WindowGroup {
            NavigationStack {
                DashboardView(paired: false, batteryPercent: 0, charging: false, signalLevel: 0)
                    .navigationTitle("PocketBridge")
            }
        }
    }
}
