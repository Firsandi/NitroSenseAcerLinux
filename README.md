# ⚡ NitroSense Linux (nitrosense-rs)

> **A blazing-fast, ultra-lightweight reverse-engineered Acer NitroSense & PredatorSense implementation for Linux, written in 100% Rust.**

![NitroSense Status](https://img.shields.io/badge/Language-Rust-orange.svg)
![Target](https://img.shields.io/badge/Model-Acer%20Nitro%20ANV15--51-red.svg)
![Status](https://img.shields.io/badge/Status-Fully%20Functional-brightgreen.svg)

---

## 🔍 Reverse Engineering Findings (from `morluto/rea`)

Through static & binary analysis of the official Windows installer (`Nitro Sense_Acer_5.2.84_W11x64_A.zip`):
1. **Windows NitroSense Architecture**:
   - Wrapped in an Electron / UWP app bundle consuming **~600MB - 1GB RAM**.
   - Front-end talks over TCP (`127.0.0.1:46933`) using a packet structure:
     `"ACER" (4-byte magic) + uint32 opcode + JSON payload`.
   - Backend service (`AcerAgentService.exe` & `AcerHardwareService.exe`) calls kernel ACPI WMI devices (`WMID_GUID3`, `WMID_GUID4`, `WMID_GUID5`).
2. **Reverse-Engineered WMI & EC Functions**:
   - **Fan Speed Calculation**:
     ```c
     u64 fan_val = (((percentage * 25600) / 100) & 0xFF00) + fan_index;
     // fan_index: 1 for CPU, 4 for GPU
     // Modes: Auto = 0x410009, Max = 0x820009, Custom = 0xC30009
     ```
   - **Operating Profiles**: Quiet, Balanced (Default), Performance, Turbo.
   - **Battery Care**: 80% charging threshold limiter via ACPI battery health mask.
   - **Backlight Timeout**: 30-second keyboard sleep timeout toggle.

---

## 🚀 Features in `nitrosense-linux`

- **Native Desktop GUI Application**: Cyberpunk HUD dark interface with zero emojis, SVG vector icons, real-time circular tachometers, and live telemetry.
- **Ultra Lightweight**: Consumes **< 25MB RAM** and negligible CPU (vs 600MB–1GB on Windows).
- **Interactive TUI Dashboard**: Rich terminal interface with gauges, dials, temperature meters, and hotkey controls.
- **Hardware Telemetry**:
  - CPU Package & Core temperatures (via `coretemp`), frequency, and real-time usage %.
  - NVIDIA Dedicated GPU temperatures, utilization, VRAM usage, power draw, and graphics clock.
  - Authentic Fan RPMs via kernel `hwmon` driver (`fan1_input` & `fan2_input`).
  - System RAM usage & Battery health / charging status.
- **Operating Profiles (Acer Nitro ANV15-51)**:
  - `Quiet` (Eco / Battery Saving)
  - `Balanced` (Default Daily Multitasking)
  - `Performance` (High Clock Frequency & Aggressive Cooling)
- **Fan Control**:
  - `Auto`: Automatic BIOS/EC thermal curve
  - `Max`: 100% full fan speed (~7800 RPM)
  - `Custom`: Independent CPU and GPU fan percentage sliders (20% - 100%)
  - `Sync`: Synchronized dual-fan adjustments
- **Battery Care**:
  - 80% maximum charge limiter toggle to extend battery lifespan.
- **Keyboard Backlight**:
  - 30-second idle auto-sleep toggle.

---

## 💻 Quick Start & Usage

### 1. Native Desktop GUI (Recommended)
Launch the native desktop window:
```bash
nitrosense gui
```
Or launch from your Linux desktop application menu (**Acer NitroSense**).

**Keyboard Shortcuts in GUI**:
- `[1]` : Quiet Mode
- `[2]` : Balanced Mode
- `[3]` : Performance Mode
- `[A]` : All Fans Auto
- `[X]` : All Fans Max (100%)
- `[B]` : Toggle 80% Battery Care Limiter
- `[K]` : Toggle Keyboard 30s Backlight Timeout

---

### 2. Interactive Terminal Dashboard (TUI)
```bash
nitrosense tui
```

---

### 3. Command Line Interface (CLI)

#### Check Real-time Telemetry:
```bash
nitrosense status
# or JSON format
nitrosense status --json
```

#### Change Operating Mode:
```bash
nitrosense mode quiet
nitrosense mode balanced
nitrosense mode performance
```

#### Control Fans:
```bash
nitrosense fan auto
nitrosense fan max
nitrosense fan custom --cpu 75 --gpu 80
```

#### Battery Health Limiter (80% Cap):
```bash
nitrosense battery --limit 80
nitrosense battery --limit 100
```

#### Keyboard Backlight:
```bash
nitrosense backlight --timeout on
nitrosense backlight --timeout off
```

---

## 🛠 Compilation & Installation

```bash
cd nitrosense-linux
cargo build --release
cp target/release/nitrosense ~/.cargo/bin/nitrosense
```
Ensure `~/.cargo/bin` is in your `PATH`.

