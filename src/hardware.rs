use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub model: String,
    pub vendor: String,
    pub cpu_model: String,
    pub gpu_model: String,
    pub kernel: String,
    pub driver_installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareMetrics {
    pub cpu_temp_c: f32,
    pub cpu_usage_pct: f32,
    pub cpu_freq_mhz: u32,
    pub gpu_temp_c: f32,
    pub gpu_usage_pct: f32,
    pub gpu_clock_mhz: u32,
    pub vram_used_mb: u32,
    pub vram_total_mb: u32,
    pub gpu_power_w: f32,
    pub ram_used_mb: u32,
    pub ram_total_mb: u32,
    pub battery_pct: u8,
    pub battery_charging: bool,
    pub ac_plugged: bool,
    pub cpu_fan_speed_pct: u8,
    pub gpu_fan_speed_pct: u8,
    pub cpu_fan_rpm: u16,
    pub gpu_fan_rpm: u16,
    pub cpu_fan_mode: String,
    pub gpu_fan_mode: String,
    pub current_profile: String,
    pub battery_limiter_active: bool,
    pub backlight_timeout_active: bool,
    pub usb_charging_active: bool,
}

pub struct HardwareManager {
    prev_cpu_total: u64,
    prev_cpu_idle: u64,
    pub active_profile: String,
    pub cpu_fan_mode: String,
    pub gpu_fan_mode: String,
    pub cpu_fan_speed: u8,
    pub gpu_fan_speed: u8,
    pub battery_limiter: bool,
    pub backlight_timeout: bool,
    pub usb_charging: bool,
}

impl HardwareManager {
    pub fn new() -> Self {
        let (total, idle) = Self::read_cpu_raw_stats().unwrap_or((0, 0));
        let active_profile = Self::read_platform_profile();
        let battery_limiter = Self::read_battery_limiter();
        let backlight_timeout = Self::read_backlight_timeout();
        let usb_charging = Self::read_usb_charging();
        Self {
            prev_cpu_total: total,
            prev_cpu_idle: idle,
            active_profile,
            cpu_fan_mode: "auto".to_string(),
            gpu_fan_mode: "auto".to_string(),
            cpu_fan_speed: 50,
            gpu_fan_speed: 50,
            battery_limiter,
            backlight_timeout,
            usb_charging,
        }
    }

    pub fn get_system_info() -> SystemInfo {
        let model = fs::read_to_string("/sys/class/dmi/id/product_name")
            .unwrap_or_else(|_| "Acer Nitro Series".to_string())
            .trim()
            .to_string();

        let vendor = fs::read_to_string("/sys/class/dmi/id/sys_vendor")
            .unwrap_or_else(|_| "Acer".to_string())
            .trim()
            .to_string();

        let kernel = fs::read_to_string("/proc/sys/kernel/osrelease")
            .unwrap_or_else(|_| "Linux".to_string())
            .trim()
            .to_string();

        let mut cpu_model = "Unknown CPU".to_string();
        if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
            for line in cpuinfo.lines() {
                if line.starts_with("model name") {
                    if let Some(pos) = line.find(':') {
                        cpu_model = line[pos + 1..].trim().to_string();
                        break;
                    }
                }
            }
        }

        let gpu_model = Self::query_gpu_name().unwrap_or_else(|| "NVIDIA Dedicated GPU".to_string());
        let driver_installed = Self::find_driver_sysfs_path().is_some();

        SystemInfo {
            model,
            vendor,
            cpu_model,
            gpu_model,
            kernel,
            driver_installed,
        }
    }

    pub fn read_metrics(&mut self) -> HardwareMetrics {
        let cpu_temp_c = Self::read_cpu_temperature();
        let cpu_usage_pct = self.calculate_cpu_usage();
        let cpu_freq_mhz = Self::read_cpu_freq_mhz();

        let (gpu_temp_c, gpu_usage_pct, gpu_clock_mhz, vram_used_mb, vram_total_mb, gpu_power_w) =
            Self::read_gpu_stats().unwrap_or((40.0, 0.0, 0, 0, 4096, 0.0));

        let (ram_used_mb, ram_total_mb) = Self::read_ram_stats();
        let (battery_pct, battery_charging, ac_plugged) = Self::read_battery_stats();

        let (cpu_rpm, gpu_rpm) = Self::read_hwmon_fan_rpms();
        let (cpu_fan_speed_pct, gpu_fan_speed_pct) = Self::calculate_fan_percentages(
            cpu_rpm,
            gpu_rpm,
            &self.cpu_fan_mode,
            self.cpu_fan_speed,
            &self.gpu_fan_mode,
            self.gpu_fan_speed,
        );
        let current_profile = self.active_profile.clone();
        let battery_limiter_active = self.battery_limiter;
        let backlight_timeout_active = self.backlight_timeout;
        let usb_charging_active = self.usb_charging;

        HardwareMetrics {
            cpu_temp_c,
            cpu_usage_pct,
            cpu_freq_mhz,
            gpu_temp_c,
            gpu_usage_pct,
            gpu_clock_mhz,
            vram_used_mb,
            vram_total_mb,
            gpu_power_w,
            ram_used_mb,
            ram_total_mb,
            battery_pct,
            battery_charging,
            ac_plugged,
            cpu_fan_speed_pct,
            gpu_fan_speed_pct,
            cpu_fan_rpm: cpu_rpm,
            gpu_fan_rpm: gpu_rpm,
            cpu_fan_mode: self.cpu_fan_mode.clone(),
            gpu_fan_mode: self.gpu_fan_mode.clone(),
            current_profile,
            battery_limiter_active,
            backlight_timeout_active,
            usb_charging_active,
        }
    }

    fn read_cpu_raw_stats() -> Option<(u64, u64)> {
        let content = fs::read_to_string("/proc/stat").ok()?;
        let first_line = content.lines().next()?;
        if !first_line.starts_with("cpu ") {
            return None;
        }
        let parts: Vec<u64> = first_line
            .split_whitespace()
            .skip(1)
            .filter_map(|s| s.parse().ok())
            .collect();
        if parts.len() < 4 {
            return None;
        }
        let idle = parts[3];
        let total: u64 = parts.iter().sum();
        Some((total, idle))
    }

    fn calculate_cpu_usage(&mut self) -> f32 {
        if let Some((total, idle)) = Self::read_cpu_raw_stats() {
            let total_delta = total.saturating_sub(self.prev_cpu_total);
            let idle_delta = idle.saturating_sub(self.prev_cpu_idle);
            self.prev_cpu_total = total;
            self.prev_cpu_idle = idle;

            if total_delta > 0 {
                let usage = 100.0 * (1.0 - (idle_delta as f32 / total_delta as f32));
                return usage.clamp(0.0, 100.0);
            }
        }
        0.0
    }

    fn read_cpu_temperature() -> f32 {
        // Search hwmon directories for coretemp or acpitz
        if let Ok(entries) = fs::read_dir("/sys/class/hwmon") {
            for entry in entries.flatten() {
                let path = entry.path();
                let name_file = path.join("name");
                if let Ok(name) = fs::read_to_string(&name_file) {
                    let name = name.trim();
                    if name == "coretemp" || name == "k10temp" || name == "acpitz_0" {
                        // Read temp1_input or highest temp
                        if let Ok(temp_str) = fs::read_to_string(path.join("temp1_input")) {
                            if let Ok(val) = temp_str.trim().parse::<f32>() {
                                return val / 1000.0;
                            }
                        }
                    }
                }
            }
        }
        45.0
    }

    fn read_cpu_freq_mhz() -> u32 {
        if let Ok(freq_str) = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq") {
            if let Ok(freq_khz) = freq_str.trim().parse::<u32>() {
                return freq_khz / 1000;
            }
        }
        0
    }

    fn query_gpu_name() -> Option<String> {
        let output = Command::new("nvidia-smi")
            .args(["--query-gpu=name", "--format=csv,noheader"])
            .output()
            .ok()?;
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
        None
    }

    fn read_gpu_stats() -> Option<(f32, f32, u32, u32, u32, f32)> {
        // query: temperature.gpu,utilization.gpu,clocks.current.graphics,memory.used,memory.total,power.draw
        let output = Command::new("nvidia-smi")
            .args([
                "--query-gpu=temperature.gpu,utilization.gpu,clocks.current.graphics,memory.used,memory.total,power.draw",
                "--format=csv,noheader,nounits",
            ])
            .output()
            .ok()?;

        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            let parts: Vec<&str> = text.trim().split(',').map(|s| s.trim()).collect();
            if parts.len() >= 6 {
                let temp: f32 = parts[0].parse().unwrap_or(0.0);
                let util: f32 = parts[1].parse().unwrap_or(0.0);
                let clock: u32 = parts[2].parse().unwrap_or(0);
                let mem_used: u32 = parts[3].parse().unwrap_or(0);
                let mem_total: u32 = parts[4].parse().unwrap_or(4096);
                let power: f32 = parts[5].parse().unwrap_or(0.0);
                return Some((temp, util, clock, mem_used, mem_total, power));
            }
        }
        None
    }

    fn read_ram_stats() -> (u32, u32) {
        let mut total_kb: u64 = 0;
        let mut avail_kb: u64 = 0;

        if let Ok(content) = fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(val) = line.split_whitespace().nth(1) {
                        total_kb = val.parse().unwrap_or(0);
                    }
                } else if line.starts_with("MemAvailable:") {
                    if let Some(val) = line.split_whitespace().nth(1) {
                        avail_kb = val.parse().unwrap_or(0);
                    }
                }
            }
        }

        let used_kb = total_kb.saturating_sub(avail_kb);
        ((used_kb / 1024) as u32, (total_kb / 1024) as u32)
    }

    fn read_battery_stats() -> (u8, bool, bool) {
        let bat_path = Path::new("/sys/class/power_supply/BAT1");
        let capacity: u8 = fs::read_to_string(bat_path.join("capacity"))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(80);

        let status = fs::read_to_string(bat_path.join("status"))
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        let charging = status == "charging";

        let ac_online: bool = fs::read_to_string("/sys/class/power_supply/ACAD/online")
            .or_else(|_| fs::read_to_string("/sys/class/power_supply/AC/online"))
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok())
            .map(|val| val == 1)
            .unwrap_or(true);

        (capacity, charging, ac_online)
    }

    pub fn find_driver_sysfs_path() -> Option<PathBuf> {
        let candidates = [
            "/sys/devices/platform/acer-wmi/nitro_sense",
            "/sys/devices/platform/acer-wmi/predator_sense",
            "/sys/module/linuwu_sense/drivers/platform:acer-wmi/acer-wmi/nitro_sense",
            "/sys/module/linuwu_sense/drivers/platform:acer-wmi/acer-wmi/predator_sense",
        ];
        for c in candidates {
            let p = PathBuf::from(c);
            if p.exists() {
                return Some(p);
            }
        }
        // Fallback: check if /sys/devices/platform/acer-wmi exists and has nitro_sense
        let fallback = PathBuf::from("/sys/devices/platform/acer-wmi/nitro_sense");
        if fallback.exists() {
            return Some(fallback);
        }
        None
    }

    /// Read real-time CPU and GPU Fan RPMs from kernel hwmon (linuwu_sense driver)
    pub fn read_hwmon_fan_rpms() -> (u16, u16) {
        if let Ok(entries) = fs::read_dir("/sys/class/hwmon") {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(name) = fs::read_to_string(path.join("name")) {
                    if name.trim() == "acer" {
                        let cpu = fs::read_to_string(path.join("fan1_input"))
                            .ok()
                            .and_then(|s| s.trim().parse::<u16>().ok())
                            .unwrap_or(0);
                        let gpu = fs::read_to_string(path.join("fan2_input"))
                            .ok()
                            .and_then(|s| s.trim().parse::<u16>().ok())
                            .unwrap_or(0);
                        return (cpu, gpu);
                    }
                }
            }
        }
        // Fallback to EC only if hwmon is not found
        crate::ec::AcerEc::read_fan_rpms().unwrap_or((0, 0))
    }

    fn calculate_fan_percentages(
        cpu_rpm: u16,
        gpu_rpm: u16,
        cpu_mode: &str,
        cpu_custom_spd: u8,
        gpu_mode: &str,
        gpu_custom_spd: u8,
    ) -> (u8, u8) {
        let calc_pct = |rpm: u16, mode: &str, custom: u8| -> u8 {
            match mode {
                "max" => 100,
                "custom" => custom.clamp(20, 100),
                _ => {
                    // auto mode: estimate from RPM
                    if rpm > 0 {
                        ((rpm as f32 / 5500.0) * 100.0).clamp(15.0, 100.0) as u8
                    } else {
                        35
                    }
                }
            }
        };

        (
            calc_pct(cpu_rpm, cpu_mode, cpu_custom_spd),
            calc_pct(gpu_rpm, gpu_mode, gpu_custom_spd),
        )
    }

    pub fn read_platform_profile() -> String {
        // 1. Check ACPI sysfs platform_profile first
        if let Ok(p) = fs::read_to_string("/sys/firmware/acpi/platform_profile") {
            let t = p.trim().to_string();
            if t == "balanced-performance" {
                return "performance".to_string();
            } else if !t.is_empty() {
                return t;
            }
        }

        // 2. Check FreeDesktop powerprofilesctl (standard in GNOME/Zorin)
        if let Ok(out) = Command::new("powerprofilesctl").arg("get").output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
                if s == "power-saver" {
                    return "quiet".to_string();
                } else if s == "performance" {
                    return "performance".to_string();
                } else if s == "balanced" {
                    return "balanced".to_string();
                }
            }
        }

        // Fallback: check CPU scaling governor
        if let Ok(gov) = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor") {
            let g = gov.trim();
            if g == "performance" {
                return "performance".to_string();
            } else if g == "powersave" {
                return "balanced".to_string();
            }
        }
        "balanced".to_string()
    }

    pub fn apply_profile(&mut self, mode: &str) -> Result<()> {
        let m = mode.to_lowercase();
        self.active_profile = m.clone();

        if m == "quiet" || m == "balanced" {
            self.cpu_fan_mode = "auto".to_string();
            self.gpu_fan_mode = "auto".to_string();
            let _ = Self::apply_hardware_fans("auto", 0, "auto", 0);
        } else if m == "performance" {
            let _ = Self::apply_hardware_fans(&self.cpu_fan_mode, self.cpu_fan_speed, &self.gpu_fan_mode, self.gpu_fan_speed);
        }

        Self::set_platform_profile(&m)
    }

    pub fn apply_battery_limiter(&mut self, enabled: bool) -> Result<()> {
        self.battery_limiter = enabled;
        Self::set_battery_limiter(enabled)
    }

    pub fn apply_backlight_timeout(&mut self, enabled: bool) -> Result<()> {
        self.backlight_timeout = enabled;
        Self::set_backlight_timeout(enabled)
    }

    pub fn apply_usb_charging(&mut self, enabled: bool) -> Result<()> {
        self.usb_charging = enabled;
        Self::set_usb_charging(enabled)
    }

    pub fn set_platform_profile(mode: &str) -> Result<()> {
        let valid_modes = ["quiet", "balanced", "performance", "low-power"];
        let target = mode.to_lowercase();
        if !valid_modes.contains(&target.as_str()) {
            anyhow::bail!("Mode must be one of: quiet, balanced, performance");
        }

        // 1. ACPI sysfs platform_profile node (linuwu_sense ACPI node)
        let acpi_profile = match target.as_str() {
            "quiet" | "low-power" => "quiet",
            "performance" => "balanced-performance",
            _ => "balanced",
        };

        let acpi_path = Path::new("/sys/firmware/acpi/platform_profile");
        if acpi_path.exists() {
            let _ = fs::write(acpi_path, acpi_profile);
        }

        // 2. FreeDesktop powerprofilesctl (CPU governors & energy preference)
        let pctl_mode = match target.as_str() {
            "quiet" | "low-power" => "power-saver",
            "performance" => "performance",
            _ => "balanced",
        };
        let _ = Command::new("powerprofilesctl")
            .args(["set", pctl_mode])
            .output();

        // 3. Adjust CPU governor if needed
        let governor = if target == "performance" {
            "performance"
        } else {
            "powersave"
        };
        let _ = Self::set_cpu_governor(governor);

        Ok(())
    }

    fn set_cpu_governor(gov: &str) -> Result<()> {
        let pattern = "/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor";
        let _ = Command::new("sh")
            .arg("-c")
            .arg(format!("for g in {}; do echo {} | sudo -n tee \"$g\" >/dev/null 2>&1; done", pattern, gov))
            .output();
        Ok(())
    }

    pub fn apply_fan_control(
        &mut self,
        target: &str,
        mode: &str,
        speed: u8,
    ) -> Result<()> {
        let m = mode.to_lowercase();
        let spd = speed.clamp(20, 100);

        match target.to_lowercase().as_str() {
            "cpu" => {
                self.cpu_fan_mode = m;
                if self.cpu_fan_mode == "custom" {
                    self.cpu_fan_speed = spd;
                }
            }
            "gpu" => {
                self.gpu_fan_mode = m;
                if self.gpu_fan_mode == "custom" {
                    self.gpu_fan_speed = spd;
                }
            }
            _ => {
                self.cpu_fan_mode = m.clone();
                self.gpu_fan_mode = m;
                if self.cpu_fan_mode == "custom" {
                    self.cpu_fan_speed = spd;
                    self.gpu_fan_speed = spd;
                }
            }
        }

        Self::apply_hardware_fans(
            &self.cpu_fan_mode,
            self.cpu_fan_speed,
            &self.gpu_fan_mode,
            self.gpu_fan_speed,
        )
    }

    pub fn apply_hardware_fans(
        cpu_mode: &str,
        cpu_spd: u8,
        gpu_mode: &str,
        gpu_spd: u8,
    ) -> Result<()> {
        let cpu_val: u8 = match cpu_mode {
            "max" => 100,
            "custom" => cpu_spd.clamp(20, 100),
            _ => 0, // auto
        };
        let gpu_val: u8 = match gpu_mode {
            "max" => 100,
            "custom" => gpu_spd.clamp(20, 100),
            _ => 0, // auto
        };

        // 1. Sysfs driver interface: /sys/devices/platform/acer-wmi/nitro_sense/fan_speed
        if let Some(base) = Self::find_driver_sysfs_path() {
            let speed_file = base.join("fan_speed");
            if speed_file.exists() {
                let payload = format!("{},{}", cpu_val, gpu_val);
                let _ = fs::write(&speed_file, payload);
                return Ok(());
            }
        }

        // 2. Embedded Controller direct hardware registers fallback
        if crate::ec::AcerEc::is_available() {
            if !crate::ec::AcerEc::is_writable() {
                let _ = crate::ec::AcerEc::ensure_permissions();
            }

            // CoolerBoost
            if cpu_val == 100 && gpu_val == 100 {
                let _ = crate::ec::AcerEc::write_byte(crate::ec::REG_COOLER_BOOST, crate::ec::COOLER_BOOST_ON);
            } else {
                let _ = crate::ec::AcerEc::write_byte(crate::ec::REG_COOLER_BOOST, crate::ec::COOLER_BOOST_OFF);
            }

            // CPU fan EC register
            match cpu_mode {
                "max" => {
                    let _ = crate::ec::AcerEc::write_byte(crate::ec::REG_CPU_FAN_MODE, crate::ec::CPU_FAN_TURBO);
                    let _ = crate::ec::AcerEc::write_byte(crate::ec::REG_CPU_FAN_SPEED, 0x64);
                }
                "custom" => {
                    let _ = crate::ec::AcerEc::write_byte(crate::ec::REG_CPU_FAN_MODE, crate::ec::CPU_FAN_MANUAL);
                    let _ = crate::ec::AcerEc::write_byte(crate::ec::REG_CPU_FAN_SPEED, cpu_spd);
                }
                _ => {
                    let _ = crate::ec::AcerEc::write_byte(crate::ec::REG_CPU_FAN_MODE, crate::ec::CPU_FAN_AUTO);
                }
            }

            // GPU fan EC register
            match gpu_mode {
                "max" => {
                    let _ = crate::ec::AcerEc::write_byte(crate::ec::REG_GPU_FAN_MODE, crate::ec::GPU_FAN_TURBO);
                    let _ = crate::ec::AcerEc::write_byte(crate::ec::REG_GPU_FAN_SPEED, 0x64);
                }
                "custom" => {
                    let _ = crate::ec::AcerEc::write_byte(crate::ec::REG_GPU_FAN_MODE, crate::ec::GPU_FAN_MANUAL);
                    let _ = crate::ec::AcerEc::write_byte(crate::ec::REG_GPU_FAN_SPEED, gpu_spd);
                }
                _ => {
                    let _ = crate::ec::AcerEc::write_byte(crate::ec::REG_GPU_FAN_MODE, crate::ec::GPU_FAN_AUTO);
                }
            }
        }

        Ok(())
    }

    pub fn set_fan_speeds(cpu: u8, gpu: u8) -> Result<()> {
        let cpu_mode = if cpu == 100 { "max" } else if cpu == 0 { "auto" } else { "custom" };
        let gpu_mode = if gpu == 100 { "max" } else if gpu == 0 { "auto" } else { "custom" };
        Self::apply_hardware_fans(cpu_mode, cpu, gpu_mode, gpu)
    }

    fn read_battery_limiter() -> bool {
        if let Some(base) = Self::find_driver_sysfs_path() {
            let file = base.join("battery_limiter");
            if let Ok(content) = fs::read_to_string(file) {
                return content.trim() == "1";
            }
        }
        // Fallback: check if BAT1 has charge_control_limit_max == 80
        if let Ok(limit) = fs::read_to_string("/sys/class/power_supply/BAT1/charge_control_limit_max") {
            return limit.trim() == "80";
        }
        true
    }

    pub fn set_battery_limiter(enabled: bool) -> Result<()> {
        let val = if enabled { "1" } else { "0" };
        if let Some(base) = Self::find_driver_sysfs_path() {
            let file = base.join("battery_limiter");
            if file.exists() {
                let _ = fs::write(&file, val);
            }
        }

        let _ = crate::ec::AcerEc::set_battery_limit(enabled);

        let bat_file = "/sys/class/power_supply/BAT1/charge_control_limit_max";
        if Path::new(bat_file).exists() {
            let target_val = if enabled { "80" } else { "100" };
            let _ = fs::write(bat_file, target_val);
        }

        Ok(())
    }

    fn read_backlight_timeout() -> bool {
        if let Some(base) = Self::find_driver_sysfs_path() {
            let file = base.join("backlight_timeout");
            if let Ok(content) = fs::read_to_string(file) {
                return content.trim() == "1";
            }
        }
        true
    }

    pub fn set_backlight_timeout(enabled: bool) -> Result<()> {
        let val = if enabled { "1" } else { "0" };
        if let Some(base) = Self::find_driver_sysfs_path() {
            let file = base.join("backlight_timeout");
            if file.exists() {
                let _ = fs::write(&file, val);
            }
        }

        let _ = crate::ec::AcerEc::set_backlight_timeout(enabled);
        Ok(())
    }

    fn read_usb_charging() -> bool {
        if let Some(base) = Self::find_driver_sysfs_path() {
            let file = base.join("usb_charging");
            if let Ok(content) = fs::read_to_string(file) {
                let val = content.trim();
                return val != "0" && !val.is_empty();
            }
        }
        true
    }

    pub fn set_usb_charging(enabled: bool) -> Result<()> {
        let val = if enabled { "30" } else { "0" };
        if let Some(base) = Self::find_driver_sysfs_path() {
            let file = base.join("usb_charging");
            if file.exists() {
                let _ = fs::write(&file, val);
            }
        }
        Ok(())
    }


    pub fn set_rgb_color(_zone: u8, r: u8, g: u8, b: u8, brightness: u8) -> Result<()> {
        if let Some(base) = Self::find_driver_sysfs_path() {
            let file = base.join("four_zoned_rgb");
            if file.exists() {
                let hex_color = format!("{:02x}{:02x}{:02x}", r, g, b);
                let payload = format!("{},{},{},{},{}", hex_color, hex_color, hex_color, hex_color, brightness);
                let _ = fs::write(&file, &payload).or_else(|_| {
                    Command::new("sudo")
                        .args(["-n", "tee", file.to_str().unwrap()])
                        .output()
                        .map(|_| ())
                });
                return Ok(());
            }
        }
        anyhow::bail!("RGB interface not found in sysfs.")
    }
}

