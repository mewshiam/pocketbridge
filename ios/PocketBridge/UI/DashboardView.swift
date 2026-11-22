import SwiftUI

/// Connection Dashboard: bridge status badge, battery %, SIM signal bars
/// (blueprint Task 4 SwiftUI Dashboard).
public struct DashboardView: View {
    public var paired: Bool
    public var batteryPercent: Int
    public var charging: Bool
    public var signalLevel: Int

    public init(paired: Bool = false, batteryPercent: Int = 0, charging: Bool = false, signalLevel: Int = 0) {
        self.paired = paired
        self.batteryPercent = batteryPercent
        self.charging = charging
        self.signalLevel = signalLevel
    }

    public var body: some View {
        VStack(spacing: 16) {
            HStack {
                Circle()
                    .fill(paired ? Color(red: 0x10/255, green: 0xB9/255, blue: 0x81/255) : Color(red: 0xEF/255, green: 0x44/255, blue: 0x44/255))
                    .frame(width: 12, height: 12)
                Text(paired ? "Connected · End-to-end encrypted" : "Disconnected")
                    .font(.headline)
            }
            Text("Android bridge battery: \(batteryPercent)%\(charging ? " (charging)" : "")")
                .font(.subheadline)
            HStack(spacing: 4) {
                ForEach(0..<4, id: \.self) { bar in
                    RoundedRectangle(cornerRadius: 2)
                        .fill(bar < signalLevel ? Color(red: 0x10/255, green: 0xB9/255, blue: 0x81/255) : Color(.systemGray4))
                        .frame(width: 6, height: CGFloat(8 + bar * 6))
                }
            }
        }
        .padding()
        .background(Color(red: 0xF8/255, green: 0xFA/255, blue: 0xFC/255))
        .cornerRadius(12)
    }
}

public struct DashboardView_Previews: PreviewProvider {
    public static var previews: some View {
        DashboardView(paired: true, batteryPercent: 78, charging: true, signalLevel: 3)
    }
}
