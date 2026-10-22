---
trigger: always_on
description: Invariants for Acer NitroSense daemon HTTP socket streaming and Linux kernel sysfs hardware interfacing.
---

# Acer Hardware & Local Daemon Invariants

## 1. HTTP Socket Request Streaming
When implementing minimal TCP/HTTP handlers in Rust for WebKitGTK or browser clients:
- Never assume an entire HTTP request arrives in a single `stream.read()` buffer.
- Always read until `\r\n\r\n` is encountered to parse headers.
- Extract `Content-Length` and loop `stream.read()` until the entire payload is accumulated in memory before passing to JSON parsers.

## 2. Hardware Interfaces on Acer Nitro (ANV15-51)
- **Fan Telemetry**: Always read fan RPMs from `/sys/class/hwmon/hwmon*` where `name` is `"acer"` (`fan1_input` for CPU, `fan2_input` for GPU). Do not read raw `/dev/ec` offsets 0x15/0x16.
- **Fan Control**: Control fans via `/sys/devices/platform/acer-wmi/nitro_sense/fan_speed` by writing `"<cpu>,<gpu>"`.
  - Max Fans: `"100,100"`
  - Auto Fans: `"0,0"`
  - Custom: `"<cpu_pct>,<gpu_pct>"`
- **Thermal Profiles**: Set ACPI platform profile `/sys/firmware/acpi/platform_profile` to `"balanced-performance"` for Performance mode, `"balanced"` for Balanced, and `"quiet"` for Quiet.
- **Battery, Backlight & USB Charging**: Use sysfs nodes under `/sys/devices/platform/acer-wmi/nitro_sense/`:
  - `battery_limiter`: `1` (80% protection limit) / `0` (100% full capacity)
  - `backlight_timeout`: `1` (30s idle timeout) / `0` (always on)
  - `usb_charging`: `30` (enabled with 30% battery reserve protection) / `0` (disabled)
- **EC Access**: Do not write blind byte registers to `/dev/ec` for features handled by `linuwu_sense` to prevent hardware state corruption.
