use anyhow::{Context, Result};
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::Command;

pub const EC_DEVICE_PATH: &str = "/dev/ec";

// Acer Nitro Embedded Controller (EC) Register Addresses
pub const REG_POWER_STATUS: u64 = 0x00;
pub const REG_BATTERY_LIMIT: u64 = 0x03;
pub const REG_KB_TIMEOUT: u64 = 0x06;
pub const REG_USB_CHARGING: u64 = 0x08;
pub const REG_COOLER_BOOST: u64 = 0x10;

// CoolerBoost values
pub const COOLER_BOOST_ON: u8 = 0x01;
pub const COOLER_BOOST_OFF: u8 = 0x00;


// Fan RPM registers (16-bit values)
pub const REG_CPU_FAN_RPM_H: u64 = 0x13;
pub const REG_CPU_FAN_RPM_L: u64 = 0x14;
pub const REG_GPU_FAN_RPM_H: u64 = 0x15;
pub const REG_GPU_FAN_RPM_L: u64 = 0x16;

// Fan Mode Control registers
pub const REG_GPU_FAN_MODE: u64 = 0x21;
pub const REG_CPU_FAN_MODE: u64 = 0x22;

// Nitro Operating Mode register
pub const REG_NITRO_MODE: u64 = 0x2C;

// Custom Manual Fan Speed registers (0 - 100%)
pub const REG_CPU_FAN_SPEED: u64 = 0x37;
pub const REG_GPU_FAN_SPEED: u64 = 0x3A;

// Mode constants
pub const MODE_QUIET: u8 = 0x00;
pub const MODE_DEFAULT: u8 = 0x01;
pub const MODE_EXTREME: u8 = 0x04; // Performance / Turbo mode

// Fan Mode constants
pub const CPU_FAN_AUTO: u8 = 0x04;
pub const CPU_FAN_TURBO: u8 = 0x08;
pub const CPU_FAN_MANUAL: u8 = 0x0C;

pub const GPU_FAN_AUTO: u8 = 0x10;
pub const GPU_FAN_TURBO: u8 = 0x20;
pub const GPU_FAN_MANUAL: u8 = 0x30;

// Battery Limit constants
pub const BATTERY_LIMIT_80_ON: u8 = 0x51;
pub const BATTERY_LIMIT_100_OFF: u8 = 0x11;

// Keyboard 30s Timeout constants
pub const KB_TIMEOUT_30S_ON: u8 = 0x1E;
pub const KB_TIMEOUT_ALWAYS_ON: u8 = 0x00;

pub struct AcerEc;

impl AcerEc {
    /// Check if /dev/ec device is available and accessible
    pub fn is_available() -> bool {
        Path::new(EC_DEVICE_PATH).exists()
    }

    /// Check if /dev/ec is writable by current user
    pub fn is_writable() -> bool {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(EC_DEVICE_PATH)
            .is_ok()
    }

    /// Attempt to make /dev/ec writable for current user using pkexec or udev
    pub fn ensure_permissions() -> bool {
        if Self::is_writable() {
            return true;
        }

        // Try unlocking with pkexec
        let cmd = "chmod 0666 /dev/ec && echo 'KERNEL==\"ec\", MODE=\"0666\"' > /etc/udev/rules.d/99-acer-ec.rules && udevadm control --reload-rules && udevadm trigger 2>/dev/null || true";
        let _ = Command::new("pkexec")
            .args(["sh", "-c", cmd])
            .status();

        Self::is_writable()
    }

    /// Read a single byte from an EC register
    pub fn read_byte(address: u64) -> Result<u8> {
        let mut file = OpenOptions::new()
            .read(true)
            .open(EC_DEVICE_PATH)
            .context("Failed to open /dev/ec for reading. Run 'sudo chmod 666 /dev/ec' or launch with sudo.")?;

        file.seek(SeekFrom::Start(address))?;
        let mut buf = [0u8; 1];
        file.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    /// Write a single byte to an EC register
    pub fn write_byte(address: u64, value: u8) -> Result<()> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(EC_DEVICE_PATH)
            .context("Failed to open /dev/ec for writing. Run 'sudo chmod 666 /dev/ec' or launch with sudo.")?;

        file.seek(SeekFrom::Start(address))?;
        file.write_all(&[value])?;
        file.flush()?;
        Ok(())
    }

    /// Set Operating Mode directly on the EC hardware
    pub fn set_mode(mode: &str) -> Result<()> {
        let m = mode.to_lowercase();
        match m.as_str() {
            "performance" | "turbo" => {
                // Switch EC to Extreme mode
                let _ = Self::write_byte(REG_NITRO_MODE, MODE_EXTREME);
                // Turn on CoolerBoost
                let _ = Self::write_byte(REG_COOLER_BOOST, COOLER_BOOST_ON);
                // Spin CPU and GPU fans to Turbo / 100%
                let _ = Self::write_byte(REG_CPU_FAN_MODE, CPU_FAN_TURBO);
                let _ = Self::write_byte(REG_CPU_FAN_SPEED, 0x64);
                let _ = Self::write_byte(REG_GPU_FAN_MODE, GPU_FAN_TURBO);
                let _ = Self::write_byte(REG_GPU_FAN_SPEED, 0x64);
            }
            "quiet" => {
                let _ = Self::write_byte(REG_NITRO_MODE, MODE_QUIET);
                let _ = Self::write_byte(REG_COOLER_BOOST, COOLER_BOOST_OFF);
                let _ = Self::write_byte(REG_CPU_FAN_MODE, CPU_FAN_AUTO);
                let _ = Self::write_byte(REG_GPU_FAN_MODE, GPU_FAN_AUTO);
            }
            _ => {
                // Balanced / Default
                let _ = Self::write_byte(REG_NITRO_MODE, MODE_DEFAULT);
                let _ = Self::write_byte(REG_COOLER_BOOST, COOLER_BOOST_OFF);
                let _ = Self::write_byte(REG_CPU_FAN_MODE, CPU_FAN_AUTO);
                let _ = Self::write_byte(REG_GPU_FAN_MODE, GPU_FAN_AUTO);
            }
        }
        Ok(())
    }

    /// Set Fan Speeds directly on the EC hardware
    pub fn set_fans(mode: &str, cpu_pct: u8, gpu_pct: u8) -> Result<()> {
        match mode.to_lowercase().as_str() {
            "max" | "turbo" => {
                let _ = Self::write_byte(REG_COOLER_BOOST, COOLER_BOOST_ON);
                let _ = Self::write_byte(REG_CPU_FAN_MODE, CPU_FAN_TURBO);
                let _ = Self::write_byte(REG_CPU_FAN_SPEED, 0x64);
                let _ = Self::write_byte(REG_GPU_FAN_MODE, GPU_FAN_TURBO);
                let _ = Self::write_byte(REG_GPU_FAN_SPEED, 0x64);
            }
            "custom" => {
                let _ = Self::write_byte(REG_COOLER_BOOST, COOLER_BOOST_OFF);
                // Set manual mode
                let _ = Self::write_byte(REG_CPU_FAN_MODE, CPU_FAN_MANUAL);
                let _ = Self::write_byte(REG_CPU_FAN_SPEED, cpu_pct.min(100));

                let _ = Self::write_byte(REG_GPU_FAN_MODE, GPU_FAN_MANUAL);
                let _ = Self::write_byte(REG_GPU_FAN_SPEED, gpu_pct.min(100));
            }
            _ => {
                // Auto mode
                let _ = Self::write_byte(REG_COOLER_BOOST, COOLER_BOOST_OFF);
                let _ = Self::write_byte(REG_CPU_FAN_MODE, CPU_FAN_AUTO);
                let _ = Self::write_byte(REG_GPU_FAN_MODE, GPU_FAN_AUTO);
            }
        }
        Ok(())
    }


    /// Read real-time Fan RPMs from EC registers
    pub fn read_fan_rpms() -> Result<(u16, u16)> {
        let cpu_h = Self::read_byte(REG_CPU_FAN_RPM_H)? as u16;
        let cpu_l = Self::read_byte(REG_CPU_FAN_RPM_L)? as u16;
        let gpu_h = Self::read_byte(REG_GPU_FAN_RPM_H)? as u16;
        let gpu_l = Self::read_byte(REG_GPU_FAN_RPM_L)? as u16;

        let cpu_rpm = (cpu_h << 8) | cpu_l;
        let gpu_rpm = (gpu_h << 8) | gpu_l;
        Ok((cpu_rpm, gpu_rpm))
    }

    /// Set Battery Care limiter (80% or 100%)
    pub fn set_battery_limit(enable_80: bool) -> Result<()> {
        let val = if enable_80 {
            BATTERY_LIMIT_80_ON
        } else {
            BATTERY_LIMIT_100_OFF
        };
        Self::write_byte(REG_BATTERY_LIMIT, val)?;
        Ok(())
    }

    /// Set Keyboard Backlight 30s timeout
    pub fn set_backlight_timeout(enable_30s: bool) -> Result<()> {
        let val = if enable_30s {
            KB_TIMEOUT_30S_ON
        } else {
            KB_TIMEOUT_ALWAYS_ON
        };
        Self::write_byte(REG_KB_TIMEOUT, val)?;
        Ok(())
    }
}
