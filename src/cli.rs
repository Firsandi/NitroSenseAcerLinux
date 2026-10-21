use crate::config::NitroConfig;
use crate::hardware::{HardwareManager, HardwareMetrics, SystemInfo};
use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::process::Command;

#[derive(Parser, Debug)]
#[command(
    name = "nitrosense",
    author = "Firsandia",
    version = "0.1.0",
    about = "⚡ Ultra-fast & lightweight NitroSense controller for Acer Nitro / Predator on Linux"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Launch the native graphical NitroSense Desktop Application
    Gui,

    /// Launch the interactive NitroSense terminal dashboard
    Tui,

    /// Show real-time hardware status and telemetry
    Status {
        /// Output formatted as JSON
        #[arg(short, long)]
        json: bool,
    },

    /// Switch thermal and performance operating mode
    Mode {
        /// Mode name: quiet, balanced, performance
        name: String,
    },

    /// Control fan behavior and speeds
    Fan {
        /// Fan mode: auto, max, custom
        mode: String,

        /// Custom CPU fan speed percentage (20 - 100)
        #[arg(long)]
        cpu: Option<u8>,

        /// Custom GPU fan speed percentage (20 - 100)
        #[arg(long)]
        gpu: Option<u8>,
    },

    /// Manage battery health and 80% charging limiter
    Battery {
        /// Set charge limit threshold (80 or 100)
        #[arg(long)]
        limit: Option<u8>,
    },

    /// Manage keyboard backlight settings
    Backlight {
        /// Set 30s idle sleep timeout (on or off)
        #[arg(long)]
        timeout: Option<String>,
    },

    /// Set 4-zone RGB keyboard colors
    Rgb {
        /// Hex color (e.g. ff0000 for red, 00ffcc for cyan)
        #[arg(long, default_value = "00e5ff")]
        color: String,

        /// Brightness percentage (0 - 100)
        #[arg(long, default_value_t = 100)]
        brightness: u8,
    },

    /// Setup and install the Acer WMI / linuwu-sense DKMS kernel driver
    SetupDriver,

    /// Unlock /dev/ec hardware permissions for fans without sudo
    Unlock,
}

pub fn handle_cli(cli: Cli) -> Result<()> {
    match cli.command {
        None | Some(Commands::Gui) => {
            crate::server::start_gui_app()?;
        }
        Some(Commands::Unlock) => {
            println!("{}", "Unlocking /dev/ec hardware fan controller...".cyan());
            if crate::ec::AcerEc::ensure_permissions() {
                println!("{}", "✔ Successfully unlocked /dev/ec! Hardware fan control is now active.".green().bold());
            } else {
                println!("{}", "Could not unlock automatically. Please run: sudo chmod 666 /dev/ec".yellow());
            }
        }

        Some(Commands::Tui) => {
            let mut app = crate::tui::TuiApp::new();
            app.run()?;
        }
        Some(Commands::Status { json }) => {
            let mut hw = HardwareManager::new();
            let sys = HardwareManager::get_system_info();
            let metrics = hw.read_metrics();

            if json {
                let out = serde_json::json!({
                    "system": sys,
                    "metrics": metrics,
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                print_status_human(&sys, &metrics);
            }
        }
        Some(Commands::Mode { name }) => {
            let mut cfg = NitroConfig::load();
            let target = name.to_lowercase();
            println!("{}", format!("Applying operating mode: {}...", target).cyan());
            HardwareManager::set_platform_profile(&target)?;
            cfg.mode = target.clone();
            let _ = cfg.save();
            println!("{}", format!("✔ Operating mode successfully switched to: {}", target.to_uppercase()).green().bold());
        }
        Some(Commands::Fan { mode, cpu, gpu }) => {
            let mut cfg = NitroConfig::load();
            let m = mode.to_lowercase();
            match m.as_str() {
                "auto" => {
                    HardwareManager::set_fan_speeds(0, 0)?;
                    cfg.fan_mode = "auto".to_string();
                    let _ = cfg.save();
                    println!("{}", "✔ Fans set to AUTO mode".green().bold());
                }
                "max" => {
                    HardwareManager::set_fan_speeds(100, 100)?;
                    cfg.fan_mode = "max".to_string();
                    let _ = cfg.save();
                    println!("{}", "✔ Fans set to MAXIMUM (100%) mode".red().bold());
                }
                "custom" => {
                    let cpu_spd = cpu.unwrap_or(cfg.cpu_fan_speed);
                    let gpu_spd = gpu.unwrap_or(cfg.gpu_fan_speed);
                    HardwareManager::set_fan_speeds(cpu_spd, gpu_spd)?;
                    cfg.fan_mode = "custom".to_string();
                    cfg.cpu_fan_speed = cpu_spd;
                    cfg.gpu_fan_speed = gpu_spd;
                    let _ = cfg.save();
                    println!("{}", format!("✔ Custom fan speeds applied: CPU={}%, GPU={}%", cpu_spd, gpu_spd).green().bold());
                }
                _ => {
                    eprintln!("{}", "Error: Invalid fan mode. Choose 'auto', 'max', or 'custom'.".red());
                }
            }
        }
        Some(Commands::Battery { limit }) => {
            let mut cfg = NitroConfig::load();
            if let Some(l) = limit {
                let enable = l <= 80;
                HardwareManager::set_battery_limiter(enable)?;
                cfg.battery_limiter = enable;
                let _ = cfg.save();
                println!(
                    "{}",
                    format!(
                        "✔ Battery Care limiter set to: {}% ({})",
                        if enable { 80 } else { 100 },
                        if enable { "Health Protection Active" } else { "Full Charge" }
                    )
                    .green()
                    .bold()
                );
            } else {
                println!(
                    "Battery 80% Health Limiter is currently: {}",
                    if cfg.battery_limiter { "ENABLED".green().bold() } else { "DISABLED".yellow() }
                );
            }
        }
        Some(Commands::Backlight { timeout }) => {
            let mut cfg = NitroConfig::load();
            if let Some(t) = timeout {
                let enable = t == "on" || t == "1" || t == "true";
                HardwareManager::set_backlight_timeout(enable)?;
                cfg.backlight_timeout = enable;
                let _ = cfg.save();
                println!(
                    "{}",
                    format!("✔ Keyboard 30s sleep timeout set to: {}", if enable { "ON" } else { "OFF" })
                        .green()
                        .bold()
                );
            } else {
                println!(
                    "Keyboard 30s Timeout is currently: {}",
                    if cfg.backlight_timeout { "ON (Auto-Sleep)".green().bold() } else { "OFF (Always On)".yellow() }
                );
            }
        }
        Some(Commands::Rgb { color, brightness }) => {
            let clean = color.trim_start_matches('#');
            if clean.len() == 6 {
                let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(230);
                let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(255);
                HardwareManager::set_rgb_color(0, r, g, b, brightness)?;
                println!("{}", format!("✔ Keyboard RGB color set to #{} at {}% brightness", clean, brightness).green().bold());
            } else {
                eprintln!("{}", "Error: Hex color must be 6 characters (e.g. ff0000)".red());
            }
        }
        Some(Commands::SetupDriver) => {
            run_driver_setup()?;
        }
    }
    Ok(())
}

fn print_status_human(sys: &SystemInfo, m: &HardwareMetrics) {
    println!("{}", "=========================================================".red());
    println!("{}", format!("  ⚡ ACER NITROSENSE LINUX - {}", sys.model).red().bold());
    println!("{}", "=========================================================".red());
    println!("{:<20}: {}", "Laptop Model".bold(), sys.model.cyan());
    println!("{:<20}: {}", "CPU Processor".bold(), sys.cpu_model);
    println!("{:<20}: {}", "Graphics GPU".bold(), sys.gpu_model.green());
    println!("{:<20}: {}", "Linux Kernel".bold(), sys.kernel);
    println!("{:<20}: {}", "Driver Status".bold(), if sys.driver_installed { "Installed & Loaded".green() } else { "Native Sysfs Fallback (Run 'setup-driver' for full WMI)".yellow() });
    println!("{}", "---------------------------------------------------------".black().bold());
    println!("{:<20}: {:.1}°C ({:.0}% usage, {} MHz)", "CPU Status".bold(), m.cpu_temp_c, m.cpu_usage_pct, m.cpu_freq_mhz);
    println!("{:<20}: {:.1}°C ({:.0}% usage, {} MHz, {:.1} W)", "GPU Status".bold(), m.gpu_temp_c, m.gpu_usage_pct, m.gpu_clock_mhz, m.gpu_power_w);
    println!("{:<20}: {} / {} MB", "GPU VRAM".bold(), m.vram_used_mb, m.vram_total_mb);
    println!("{:<20}: {:.1} / {:.1} GB", "System RAM".bold(), m.ram_used_mb as f32 / 1024.0, m.ram_total_mb as f32 / 1024.0);
    println!("{:<20}: {}% ({})", "Battery Level".bold(), m.battery_pct, if m.ac_plugged { "AC Connected" } else { "Discharging" });
    println!("{:<20}: {}", "Operating Mode".bold(), m.current_profile.to_uppercase().cyan().bold());
    println!("{:<20}: CPU {}% / GPU {}%", "Fan Speeds".bold(), m.cpu_fan_speed_pct, m.gpu_fan_speed_pct);
    println!("{:<20}: {}", "80% Battery Limit".bold(), if m.battery_limiter_active { "Active (80% Cap)".green() } else { "Disabled (100% Full)".yellow() });
    println!("{:<20}: {}", "Backlight Timeout".bold(), if m.backlight_timeout_active { "Enabled (30s sleep)".green() } else { "Always On".yellow() });
    println!("{}", "=========================================================".red());
}

fn run_driver_setup() -> Result<()> {
    println!("{}", "=== Acer Nitro Linux Driver Setup ===".cyan().bold());
    println!("Checking DKMS and kernel headers...");

    let dkms_check = Command::new("which").arg("dkms").output();
    let has_dkms = dkms_check.map(|o| o.status.success()).unwrap_or(false);

    if !has_dkms {
        println!("{}", "DKMS is not installed. To build the kernel module, run:".yellow());
        println!("  sudo apt install dkms linux-headers-$(uname -r)");
        return Ok(());
    }

    println!("{}", "DKMS found! Preparing driver build...".green());
    println!("Driver repository is cloned at 'scratch_linuwu/'.");
    println!("To install the driver module with auto-rebuild on kernel update, run:");
    println!("  cd /home/firsandia/Desktop/ProjekFomo1/scratch_linuwu && sudo make install");
    println!("After installation, full WMI fan curves, RGB, and battery limiters will be unlocked!");
    Ok(())
}
