import SwiftUI

@main
struct PocketBridgeApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

struct ContentView: View {
    var body: some View {
        NavigationStack {
            VStack(spacing: 12) {
                Image(systemName: "wave.3.left.circle.fill")
                    .font(.system(size: 48))
                    .foregroundStyle(Color(red: 0.06, green: 0.73, blue: 0.51))
                Text("PocketBridge")
                    .font(.title2.bold())
                Text("Scan your Android Bridge QR code to establish an encrypted BLE session.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal)
            }
            .navigationTitle("PocketBridge")
        }
    }
}
