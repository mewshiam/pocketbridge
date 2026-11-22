# PocketBridge Device Compatibility

Crowdsourced matrix (blueprint §10): report via the device-compatibility
issue template. The Android **Audio Self-Test Wizard** probes each tier and
locks the best one per device.

## Known behaviors by OEM

| OEM | Voice capture | Notes |
|---|---|---|
| Stock Android (Pixel) | Tier 1 usually passes | `VOICE_COMMUNICATION` capture works |
| Samsung (One UI) | Tier 1 or Tier 2 | Sleeping Apps must be disabled for the service |
| Xiaomi (HyperOS/MIUI) | Tier 2 or Tier 3 (Shizuku) | Autostart + battery saver "No restrictions" required |
| Huawei/Honor (EMUI) | Tier 3 (Shizuku) recommended | Aggressive background app termination |

## Background survival checklist

1. Foreground service `connectedDevice|phoneCall` (always-on notification).
2. `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` prompt on first run.
3. In-app OEM guides (EN/FA localized): Xiaomi HyperOS, Samsung Sleeping Apps, Huawei/Honor.

## Reporting a device

Open an issue with the **Device compatibility** template: model, ROM version,
self-test results per tier, and the locked strategy.
