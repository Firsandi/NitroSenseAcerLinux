use crate::config::NitroConfig;
use crate::hardware::{HardwareManager, HardwareMetrics, SystemInfo};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Gauge, Paragraph,
    },
    Frame, Terminal,
};
use std::io::stdout;
use std::time::{Duration, Instant};

const COLOR_NITRO_RED: Color = Color::Rgb(255, 36, 52);
const COLOR_CYAN: Color = Color::Rgb(0, 235, 255);
const COLOR_GOLD: Color = Color::Rgb(255, 175, 0);
const COLOR_GREEN: Color = Color::Rgb(40, 220, 100);
const COLOR_PANEL: Color = Color::Rgb(22, 26, 35);
const COLOR_MUTED: Color = Color::Rgb(120, 130, 150);

pub struct TuiApp {
    hw: HardwareManager,
    sys_info: SystemInfo,
    metrics: HardwareMetrics,
    config: NitroConfig,
    status_message: String,
    last_tick: Instant,
}

impl TuiApp {
    pub fn new() -> Self {
        let mut hw = HardwareManager::new();
        let sys_info = HardwareManager::get_system_info();
        let metrics = hw.read_metrics();
        let config = NitroConfig::load();

        Self {
            hw,
            sys_info,
            metrics,
            config,
            status_message: "Ready. Use [1-4] to switch mode, [A/X/C] for fans, [Q] to quit.".to_string(),
            last_tick: Instant::now(),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let tick_rate = Duration::from_millis(1000);

        loop {
            terminal.draw(|f| self.render_ui(f))?;

            let timeout = tick_rate.saturating_sub(self.last_tick.elapsed());
            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            KeyCode::Char('1') => {
                                self.apply_profile("quiet");
                            }
                            KeyCode::Char('2') => {
                                self.apply_profile("balanced");
                            }
                            KeyCode::Char('3') => {
                                self.apply_profile("performance");
                            }
                            KeyCode::Char('4') => {
                                self.apply_profile("turbo");
                            }
                            KeyCode::Char('a') | KeyCode::Char('A') => {
                                self.config.fan_mode = "auto".to_string();
                                let _ = HardwareManager::set_fan_speeds(0, 0);
                                self.status_message = "Fan mode set to: AUTO".to_string();
                                let _ = self.config.save();
                            }
                            KeyCode::Char('x') | KeyCode::Char('X') => {
                                self.config.fan_mode = "max".to_string();
                                let _ = HardwareManager::set_fan_speeds(100, 100);
                                self.status_message = "Fan mode set to: MAXIMUM (100%)".to_string();
                                let _ = self.config.save();
                            }
                            KeyCode::Char('+') | KeyCode::Char('=') => {
                                self.config.cpu_fan_speed = (self.config.cpu_fan_speed + 10).min(100);
                                self.config.fan_mode = "custom".to_string();
                                let _ = HardwareManager::set_fan_speeds(self.config.cpu_fan_speed, self.config.gpu_fan_speed);
                                self.status_message = format!("CPU Fan speed: {}%", self.config.cpu_fan_speed);
                                let _ = self.config.save();
                            }
                            KeyCode::Char('-') | KeyCode::Char('_') => {
                                self.config.cpu_fan_speed = self.config.cpu_fan_speed.saturating_sub(10).max(20);
                                self.config.fan_mode = "custom".to_string();
                                let _ = HardwareManager::set_fan_speeds(self.config.cpu_fan_speed, self.config.gpu_fan_speed);
                                self.status_message = format!("CPU Fan speed: {}%", self.config.cpu_fan_speed);
                                let _ = self.config.save();
                            }
                            KeyCode::Char(']') => {
                                self.config.gpu_fan_speed = (self.config.gpu_fan_speed + 10).min(100);
                                self.config.fan_mode = "custom".to_string();
                                let _ = HardwareManager::set_fan_speeds(self.config.cpu_fan_speed, self.config.gpu_fan_speed);
                                self.status_message = format!("GPU Fan speed: {}%", self.config.gpu_fan_speed);
                                let _ = self.config.save();
                            }
                            KeyCode::Char('[') => {
                                self.config.gpu_fan_speed = self.config.gpu_fan_speed.saturating_sub(10).max(20);
                                self.config.fan_mode = "custom".to_string();
                                let _ = HardwareManager::set_fan_speeds(self.config.cpu_fan_speed, self.config.gpu_fan_speed);
                                self.status_message = format!("GPU Fan speed: {}%", self.config.gpu_fan_speed);
                                let _ = self.config.save();
                            }
                            KeyCode::Char('b') | KeyCode::Char('B') => {
                                self.config.battery_limiter = !self.config.battery_limiter;
                                let _ = HardwareManager::set_battery_limiter(self.config.battery_limiter);
                                self.status_message = format!(
                                    "Battery 80% Limiter: {}",
                                    if self.config.battery_limiter { "ENABLED" } else { "DISABLED" }
                                );
                                let _ = self.config.save();
                            }
                            KeyCode::Char('k') | KeyCode::Char('K') => {
                                self.config.backlight_timeout = !self.config.backlight_timeout;
                                let _ = HardwareManager::set_backlight_timeout(self.config.backlight_timeout);
                                self.status_message = format!(
                                    "Keyboard 30s Timeout: {}",
                                    if self.config.backlight_timeout { "ON" } else { "OFF" }
                                );
                                let _ = self.config.save();
                            }
                            _ => {}
                        }
                    }
                }
            }

            if self.last_tick.elapsed() >= tick_rate {
                self.metrics = self.hw.read_metrics();
                self.last_tick = Instant::now();
            }
        }

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        Ok(())
    }

    fn apply_profile(&mut self, profile: &str) {
        self.config.mode = profile.to_string();
        if let Err(e) = HardwareManager::set_platform_profile(profile) {
            self.status_message = format!("Mode set failed: {}", e);
        } else {
            self.status_message = format!("Switched to {} Mode!", profile.to_uppercase());
        }
        let _ = self.config.save();
    }

    fn render_ui(&self, f: &mut Frame) {
        let size = f.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Length(4), // Mode cards
                Constraint::Min(14),   // Main Gauges (CPU, GPU, RAM/Controls)
                Constraint::Length(3), // Status message
                Constraint::Length(3), // Hotkey footer
            ])
            .split(size);

        self.render_header(f, chunks[0]);
        self.render_modes(f, chunks[1]);
        self.render_main_panels(f, chunks[2]);
        self.render_status_bar(f, chunks[3]);
        self.render_footer(f, chunks[4]);
    }

    fn render_header(&self, f: &mut Frame, area: Rect) {
        let pwr_status = if self.metrics.ac_plugged {
            if self.metrics.battery_charging {
                format!("⚡ AC CHARGING [{}%]", self.metrics.battery_pct)
            } else {
                format!("🔌 AC CONNECTED [{}%]", self.metrics.battery_pct)
            }
        } else {
            format!("🔋 BATTERY [{}%]", self.metrics.battery_pct)
        };

        let title = Line::from(vec![
            Span::styled(" █ NITRO", Style::default().fg(COLOR_NITRO_RED).add_modifier(Modifier::BOLD)),
            Span::styled("SENSE ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("LINUX ", Style::default().fg(COLOR_CYAN).add_modifier(Modifier::BOLD)),
            Span::styled(format!("│ {} ", self.sys_info.model), Style::default().fg(COLOR_GOLD)),
            Span::styled(format!("│ Kernel: {} ", self.sys_info.kernel), Style::default().fg(COLOR_MUTED)),
            Span::styled(format!("│ {}", pwr_status), Style::default().fg(COLOR_GREEN).add_modifier(Modifier::BOLD)),
        ]);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_NITRO_RED));

        let p = Paragraph::new(title).block(block).alignment(Alignment::Center);
        f.render_widget(p, area);
    }

    fn render_modes(&self, f: &mut Frame, area: Rect) {
        let mode_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(area);

        let modes = [
            ("1. QUIET", "quiet", "Eco / Low Fan Noise", Color::Green),
            ("2. BALANCED", "balanced", "Default Daily Profile", COLOR_CYAN),
            ("3. PERFORMANCE", "performance", "Boost Clock & Fan", COLOR_GOLD),
            ("4. TURBO", "turbo", "Max Clocks & 100% Fan", COLOR_NITRO_RED),
        ];

        let active_mode = self.config.mode.to_lowercase();

        for (i, &(title, id, desc, color)) in modes.iter().enumerate() {
            let is_active = active_mode == id;

            let border_style = if is_active {
                Style::default().fg(color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(COLOR_MUTED)
            };

            let bg = if is_active {
                COLOR_PANEL
            } else {
                Color::Reset
            };

            let text = vec![
                Line::from(vec![
                    Span::styled(
                        if is_active { "▶ " } else { "  " },
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(title, Style::default().fg(if is_active { color } else { Color::White }).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(Span::styled(desc, Style::default().fg(COLOR_MUTED))),
            ];

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(if is_active { BorderType::Thick } else { BorderType::Plain })
                .border_style(border_style)
                .style(Style::default().bg(bg));

            let p = Paragraph::new(text).block(block).alignment(Alignment::Left);
            f.render_widget(p, mode_cols[i]);
        }
    }

    fn render_main_panels(&self, f: &mut Frame, area: Rect) {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(34), // CPU & Fan
                Constraint::Percentage(33), // GPU & Fan
                Constraint::Percentage(33), // RAM & Settings
            ])
            .split(area);

        // --- CPU Panel ---
        self.render_cpu_panel(f, cols[0]);

        // --- GPU Panel ---
        self.render_gpu_panel(f, cols[1]);

        // --- RAM & Controls Panel ---
        self.render_controls_panel(f, cols[2]);
    }

    fn render_cpu_panel(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" CPU & Fan Control ")
            .title_style(Style::default().fg(COLOR_NITRO_RED).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_NITRO_RED));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Model
                Constraint::Length(2), // Temp Gauge
                Constraint::Length(2), // Usage Gauge
                Constraint::Length(1), // Freq
                Constraint::Length(1), // Separator
                Constraint::Length(2), // Fan Gauge
                Constraint::Length(2), // Fan controls info
            ])
            .split(inner);

        // Model line
        let cpu_short = if self.sys_info.cpu_model.len() > 30 {
            &self.sys_info.cpu_model[..30]
        } else {
            &self.sys_info.cpu_model
        };
        let p_name = Paragraph::new(Line::from(vec![
            Span::styled("Model: ", Style::default().fg(COLOR_MUTED)),
            Span::styled(cpu_short, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]));
        f.render_widget(p_name, rows[0]);

        // CPU Temperature
        let temp_color = if self.metrics.cpu_temp_c < 65.0 {
            COLOR_GREEN
        } else if self.metrics.cpu_temp_c < 85.0 {
            COLOR_GOLD
        } else {
            COLOR_NITRO_RED
        };

        let temp_pct = ((self.metrics.cpu_temp_c / 100.0) * 100.0).clamp(0.0, 100.0) as u16;
        let g_temp = Gauge::default()
            .block(Block::default().title("Temperature"))
            .gauge_style(Style::default().fg(temp_color))
            .percent(temp_pct)
            .label(format!("{:.1}°C", self.metrics.cpu_temp_c));
        f.render_widget(g_temp, rows[1]);

        // CPU Usage
        let g_usage = Gauge::default()
            .block(Block::default().title("Utilization"))
            .gauge_style(Style::default().fg(COLOR_CYAN))
            .percent(self.metrics.cpu_usage_pct.clamp(0.0, 100.0) as u16)
            .label(format!("{:.1}%", self.metrics.cpu_usage_pct));
        f.render_widget(g_usage, rows[2]);

        // CPU Frequency
        let p_freq = Paragraph::new(Line::from(vec![
            Span::styled("Core Frequency: ", Style::default().fg(COLOR_MUTED)),
            Span::styled(format!("{} MHz", self.metrics.cpu_freq_mhz), Style::default().fg(COLOR_GOLD).add_modifier(Modifier::BOLD)),
        ]));
        f.render_widget(p_freq, rows[3]);

        // Fan Speed Gauge
        let fan_pct = match self.config.fan_mode.as_str() {
            "max" => 100,
            "custom" => self.config.cpu_fan_speed as u16,
            _ => (40 + (self.metrics.cpu_temp_c as u16).saturating_sub(45)).min(100),
        };

        let g_fan = Gauge::default()
            .block(Block::default().title(format!("CPU Fan [{}]", self.config.fan_mode.to_uppercase())))
            .gauge_style(Style::default().fg(COLOR_CYAN))
            .percent(fan_pct)
            .label(format!("{}%", fan_pct));
        f.render_widget(g_fan, rows[5]);

        let p_fan_hint = Paragraph::new(Line::from(vec![
            Span::styled("Adjust: [+/-] Speed | [A] Auto | [X] Max", Style::default().fg(COLOR_MUTED)),
        ]));
        f.render_widget(p_fan_hint, rows[6]);
    }

    fn render_gpu_panel(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" GPU & Fan Control ")
            .title_style(Style::default().fg(COLOR_CYAN).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_CYAN));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Model
                Constraint::Length(2), // Temp Gauge
                Constraint::Length(2), // Usage Gauge
                Constraint::Length(1), // Clock & Power
                Constraint::Length(1), // Separator
                Constraint::Length(2), // Fan Gauge
                Constraint::Length(2), // Fan controls info
            ])
            .split(inner);

        // Model
        let p_name = Paragraph::new(Line::from(vec![
            Span::styled("Model: ", Style::default().fg(COLOR_MUTED)),
            Span::styled(&self.sys_info.gpu_model, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]));
        f.render_widget(p_name, rows[0]);

        // GPU Temperature
        let temp_color = if self.metrics.gpu_temp_c < 65.0 {
            COLOR_GREEN
        } else if self.metrics.gpu_temp_c < 80.0 {
            COLOR_GOLD
        } else {
            COLOR_NITRO_RED
        };

        let temp_pct = ((self.metrics.gpu_temp_c / 100.0) * 100.0).clamp(0.0, 100.0) as u16;
        let g_temp = Gauge::default()
            .block(Block::default().title("Temperature"))
            .gauge_style(Style::default().fg(temp_color))
            .percent(temp_pct)
            .label(format!("{:.1}°C", self.metrics.gpu_temp_c));
        f.render_widget(g_temp, rows[1]);

        // GPU Usage
        let g_usage = Gauge::default()
            .block(Block::default().title(format!("Utilization (VRAM: {}/{} MB)", self.metrics.vram_used_mb, self.metrics.vram_total_mb)))
            .gauge_style(Style::default().fg(COLOR_CYAN))
            .percent(self.metrics.gpu_usage_pct.clamp(0.0, 100.0) as u16)
            .label(format!("{:.0}%", self.metrics.gpu_usage_pct));
        f.render_widget(g_usage, rows[2]);

        // Clock & Power
        let p_clock = Paragraph::new(Line::from(vec![
            Span::styled("Clock: ", Style::default().fg(COLOR_MUTED)),
            Span::styled(format!("{} MHz  ", self.metrics.gpu_clock_mhz), Style::default().fg(COLOR_GOLD)),
            Span::styled("Power: ", Style::default().fg(COLOR_MUTED)),
            Span::styled(format!("{:.1} W", self.metrics.gpu_power_w), Style::default().fg(COLOR_GREEN)),
        ]));
        f.render_widget(p_clock, rows[3]);

        // GPU Fan Speed
        let gpu_fan_pct = match self.config.fan_mode.as_str() {
            "max" => 100,
            "custom" => self.config.gpu_fan_speed as u16,
            _ => (40 + (self.metrics.gpu_temp_c as u16).saturating_sub(45)).min(100),
        };

        let g_fan = Gauge::default()
            .block(Block::default().title(format!("GPU Fan [{}]", self.config.fan_mode.to_uppercase())))
            .gauge_style(Style::default().fg(COLOR_NITRO_RED))
            .percent(gpu_fan_pct)
            .label(format!("{}%", gpu_fan_pct));
        f.render_widget(g_fan, rows[5]);

        let p_fan_hint = Paragraph::new(Line::from(vec![
            Span::styled("Adjust: [[/]] Speed | [A] Auto | [X] Max", Style::default().fg(COLOR_MUTED)),
        ]));
        f.render_widget(p_fan_hint, rows[6]);
    }

    fn render_controls_panel(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" System Health & Settings ")
            .title_style(Style::default().fg(COLOR_GOLD).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_GOLD));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // RAM Gauge
                Constraint::Length(1), // RAM text
                Constraint::Length(1), // Spacer
                Constraint::Length(2), // Battery Limiter Toggle
                Constraint::Length(2), // Backlight Timeout Toggle
                Constraint::Length(2), // Driver Status
            ])
            .split(inner);

        // RAM Gauge
        let ram_pct = if self.metrics.ram_total_mb > 0 {
            ((self.metrics.ram_used_mb as f32 / self.metrics.ram_total_mb as f32) * 100.0) as u16
        } else {
            0
        };

        let g_ram = Gauge::default()
            .block(Block::default().title("Memory (RAM)"))
            .gauge_style(Style::default().fg(COLOR_GOLD))
            .percent(ram_pct)
            .label(format!("{:.1} / {:.1} GB ({}%)", self.metrics.ram_used_mb as f32 / 1024.0, self.metrics.ram_total_mb as f32 / 1024.0, ram_pct));
        f.render_widget(g_ram, rows[0]);

        // Battery Limiter
        let bat_limit_text = if self.config.battery_limiter {
            Line::from(vec![
                Span::styled("🔋 Battery Care (80% Limit): ", Style::default().fg(Color::White)),
                Span::styled("[ ACTIVE (80%) ]", Style::default().fg(COLOR_GREEN).add_modifier(Modifier::BOLD)),
                Span::styled(" (Press B to toggle)", Style::default().fg(COLOR_MUTED)),
            ])
        } else {
            Line::from(vec![
                Span::styled("🔋 Battery Care (80% Limit): ", Style::default().fg(Color::White)),
                Span::styled("[ FULL (100%) ]", Style::default().fg(COLOR_GOLD).add_modifier(Modifier::BOLD)),
                Span::styled(" (Press B to toggle)", Style::default().fg(COLOR_MUTED)),
            ])
        };
        f.render_widget(Paragraph::new(bat_limit_text), rows[3]);

        // Keyboard Backlight Timeout
        let kb_text = if self.config.backlight_timeout {
            Line::from(vec![
                Span::styled("⌨ Backlight 30s Timeout:   ", Style::default().fg(Color::White)),
                Span::styled("[ ON (Auto Sleep) ]", Style::default().fg(COLOR_CYAN).add_modifier(Modifier::BOLD)),
                Span::styled(" (Press K to toggle)", Style::default().fg(COLOR_MUTED)),
            ])
        } else {
            Line::from(vec![
                Span::styled("⌨ Backlight 30s Timeout:   ", Style::default().fg(Color::White)),
                Span::styled("[ ALWAYS ON ]", Style::default().fg(COLOR_GOLD).add_modifier(Modifier::BOLD)),
                Span::styled(" (Press K to toggle)", Style::default().fg(COLOR_MUTED)),
            ])
        };
        f.render_widget(Paragraph::new(kb_text), rows[4]);

        // Driver Status
        let driver_text = if self.sys_info.driver_installed {
            Line::from(vec![
                Span::styled("⚡ Hardware Driver: ", Style::default().fg(Color::White)),
                Span::styled("● ACER WMI / LINUWU LOADED", Style::default().fg(COLOR_GREEN).add_modifier(Modifier::BOLD)),
            ])
        } else {
            Line::from(vec![
                Span::styled("⚡ Hardware Driver: ", Style::default().fg(Color::White)),
                Span::styled("○ NATIVE SYSFS (Run 'setup-driver')", Style::default().fg(COLOR_GOLD)),
            ])
        };
        f.render_widget(Paragraph::new(driver_text), rows[5]);
    }

    fn render_status_bar(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_MUTED));

        let text = Line::from(vec![
            Span::styled(" Status: ", Style::default().fg(COLOR_CYAN).add_modifier(Modifier::BOLD)),
            Span::styled(&self.status_message, Style::default().fg(Color::White)),
        ]);

        let p = Paragraph::new(text).block(block);
        f.render_widget(p, area);
    }

    fn render_footer(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_NITRO_RED));

        let keys = Line::from(vec![
            Span::styled(" [1] ", Style::default().fg(COLOR_NITRO_RED).add_modifier(Modifier::BOLD)),
            Span::raw("Quiet "),
            Span::styled(" [2] ", Style::default().fg(COLOR_CYAN).add_modifier(Modifier::BOLD)),
            Span::raw("Balanced "),
            Span::styled(" [3] ", Style::default().fg(COLOR_GOLD).add_modifier(Modifier::BOLD)),
            Span::raw("Performance "),
            Span::styled(" [4] ", Style::default().fg(COLOR_NITRO_RED).add_modifier(Modifier::BOLD)),
            Span::raw("Turbo "),
            Span::styled("│ [A] ", Style::default().fg(COLOR_CYAN).add_modifier(Modifier::BOLD)),
            Span::raw("Auto Fan "),
            Span::styled(" [X] ", Style::default().fg(COLOR_NITRO_RED).add_modifier(Modifier::BOLD)),
            Span::raw("Max Fan "),
            Span::styled(" [+/-] ", Style::default().fg(COLOR_GOLD).add_modifier(Modifier::BOLD)),
            Span::raw("CPU Speed "),
            Span::styled(" [[/]] ", Style::default().fg(COLOR_GOLD).add_modifier(Modifier::BOLD)),
            Span::raw("GPU Speed "),
            Span::styled("│ [B] ", Style::default().fg(COLOR_GREEN).add_modifier(Modifier::BOLD)),
            Span::raw("80% Battery "),
            Span::styled(" [Q] ", Style::default().fg(COLOR_NITRO_RED).add_modifier(Modifier::BOLD)),
            Span::raw("Quit "),
        ]);

        let p = Paragraph::new(keys).block(block).alignment(Alignment::Center);
        f.render_widget(p, area);
    }
}
