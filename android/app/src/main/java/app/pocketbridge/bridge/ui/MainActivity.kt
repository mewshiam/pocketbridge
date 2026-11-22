package app.pocketbridge.bridge.ui

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import org.jetbrains.annotations.NotNull

/**
 * MainActivity — Compose entry point for the bridge app.
 *
 * Screens (blueprint §3.2 ui/): Onboarding + permission walkthrough, QR
 * pairing, Status Dashboard, Diagnostics. v1 scaffold shows the dashboard
 * shell with the brand palette.
 */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            PocketBridgeTheme {
                BridgeApp()
            }
        }
    }
}

@Composable
fun PocketBridgeTheme(content: @Composable () -> Unit) {
    // Brand palette (blueprint §2): Deep Slate, Signal Emerald, Amber,
    // Crimson, Warm Off-White.
    MaterialTheme(
        colorScheme = MaterialTheme.colorScheme.copy(
            primary = PocketBridgeColors.signalEmerald,
            background = PocketBridgeColors.warmOffWhite,
            surface = PocketBridgeColors.warmOffWhite,
            error = PocketBridgeColors.crimson,
        ),
    ) {
        content()
    }
}

@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
fun BridgeApp() {
    Scaffold(
        topBar = { TopAppBar(title = { Text("PocketBridge") }) },
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
            verticalArrangement = Arrangement.spacedBy(androidx.compose.ui.unit.Dp(8f)),
        ) {
            StatusDashboard(paired = false, advertising = false)
        }
    }
}

@Composable
fun StatusDashboard(paired: Boolean, advertising: Boolean) {
    Column {
        Text(if (paired) "Paired" else "Not paired")
        Text(if (advertising) "Advertising" else "Idle")
    }
}

@Preview
@Composable
fun BridgeAppPreview() {
    PocketBridgeTheme {
        BridgeApp()
    }
}
