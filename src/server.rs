use crate::hardware::HardwareManager;
use anyhow::Result;
use colored::Colorize;
use serde_json::Value;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const GUI_HTML: &str = include_str!("../gui/index.html");
const NITROSENSE_SVG: &str = include_str!("../gui/nitrosense.svg");
const WINDOW_PY: &str = include_str!("../gui/window.py");
const PORT: u16 = 54433;

pub fn start_gui_app() -> Result<()> {
    println!("{}", "=========================================================".red());
    println!("{}", "  ⚡ LAUNCHING ACER NITROSENSE DESKTOP APPLICATION".red().bold());
    println!("{}", "=========================================================".red());
    println!("Starting NitroSense local hardware daemon on 127.0.0.1:{}...", PORT);

    let hw_manager = Arc::new(Mutex::new(HardwareManager::new()));
    let hw_clone = Arc::clone(&hw_manager);

    // Start local HTTP server thread
    let server_bound = match TcpListener::bind(format!("127.0.0.1:{}", PORT)) {
        Ok(listener) => {
            thread::spawn(move || {
                for stream in listener.incoming() {
                    if let Ok(stream) = stream {
                        let hw = Arc::clone(&hw_clone);
                        thread::spawn(move || {
                            handle_connection(stream, hw);
                        });
                    }
                }
            });
            true
        }
        Err(_) => {
            println!("Port {} already active; reusing existing daemon.", PORT);
            false
        }
    };

    if server_bound {
        // Give server a fraction of a second to bind
        thread::sleep(Duration::from_millis(150));
    }

    let url = format!("http://127.0.0.1:{}", PORT);
    println!("Opening native desktop window for {}...", url.cyan());

    launch_gui_window(&url)?;

    Ok(())
}

fn launch_gui_window(url: &str) -> Result<()> {
    // 1. First choice: Native GTK3 WebKit2 Desktop Window
    let window_script_path = "/tmp/nitrosense_window.py";
    let _ = std::fs::write(window_script_path, WINDOW_PY);

    println!("Launching native GTK3 WebKit window...");
    let py_child = Command::new("python3")
        .args([window_script_path, url])
        .spawn();

    if let Ok(mut child) = py_child {
        thread::sleep(Duration::from_millis(400));
        match child.try_wait() {
            Ok(Some(status)) if !status.success() => {
                println!("GTK window exited with status {}. Trying browser fallback...", status);
            }
            Ok(Some(_)) => {
                // User closed the window quickly or it exited normally
                return Ok(());
            }
            _ => {
                println!("{}", "✔ Acer NitroSense Desktop Application is open!".green().bold());
                let _ = child.wait();
                return Ok(());
            }
        }
    }

    // 2. Second choice: Chromium-based standalone app window
    let browsers = [
        "brave-origin-stable",
        "brave-origin",
        "brave-browser",
        "google-chrome-stable",
        "google-chrome",
        "chromium-browser",
        "chromium",
    ];

    for browser in browsers {
        let app_arg = format!("--app={}", url);
        let res = Command::new(browser)
            .args([
                &app_arg,
                "--start-maximized",
                "--class=nitrosense",
                "--user-data-dir=/tmp/nitrosense-app-profile",
            ])
            .spawn();

        if let Ok(mut child) = res {
            println!("{}", format!("✔ Opened standalone app window using {}", browser).green().bold());
            thread::sleep(Duration::from_millis(600));
            if let Ok(None) = child.try_wait() {
                let _ = child.wait();
                return Ok(());
            } else {
                println!("{}", "NitroSense daemon running. Press Ctrl+C in terminal to stop.".cyan());
                loop {
                    thread::sleep(Duration::from_secs(2));
                }
            }
        }
    }

    // 3. Fallback: xdg-open
    println!("Launching via system default browser...");
    let _ = Command::new("xdg-open").arg(url).spawn();
    println!("{}", "NitroSense daemon active. Press Ctrl+C in terminal to stop.".cyan().bold());
    loop {
        thread::sleep(Duration::from_secs(2));
    }
}

fn read_http_request(stream: &mut TcpStream) -> Result<(String, String, String)> {
    let mut data = Vec::new();
    let mut buf = [0u8; 1024];

    // Read until headers terminate (\r\n\r\n)
    let header_end_pos;
    loop {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            anyhow::bail!("Connection closed before headers");
        }
        data.extend_from_slice(&buf[..n]);
        if let Some(pos) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end_pos = pos + 4;
            break;
        }
        if data.len() > 16384 {
            anyhow::bail!("Request headers too large");
        }
    }

    let header_str = String::from_utf8_lossy(&data[..header_end_pos]).to_string();
    let mut lines = header_str.lines();
    let first_line = lines.next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 2 {
        anyhow::bail!("Invalid HTTP request line");
    }
    let method = parts[0].to_string();
    let path = parts[1].to_string();

    let mut content_length = 0usize;
    for line in lines {
        if let Some(val) = line.to_lowercase().strip_prefix("content-length:") {
            content_length = val.trim().parse().unwrap_or(0);
        }
    }

    // Accumulate the entire body based on Content-Length
    let mut body_bytes = data[header_end_pos..].to_vec();
    while body_bytes.len() < content_length {
        let needed = content_length - body_bytes.len();
        let to_read = needed.min(buf.len());
        let n = stream.read(&mut buf[..to_read])?;
        if n == 0 {
            break;
        }
        body_bytes.extend_from_slice(&buf[..n]);
    }

    let body_str = String::from_utf8_lossy(&body_bytes).to_string();
    Ok((method, path, body_str))
}

fn handle_connection(mut stream: TcpStream, hw: Arc<Mutex<HardwareManager>>) {
    let (method, path, body) = match read_http_request(&mut stream) {
        Ok(req) => req,
        Err(_) => return,
    };

    if method == "OPTIONS" {
        send_response(&mut stream, 200, "text/plain", b"");
    } else if method == "GET" && (path == "/" || path == "/index.html") {
        send_response(&mut stream, 200, "text/html", GUI_HTML.as_bytes());
    } else if method == "GET" && (path == "/nitrosense.svg" || path == "/favicon.ico") {
        send_response(&mut stream, 200, "image/svg+xml", NITROSENSE_SVG.as_bytes());
    } else if method == "GET" && path == "/api/status" {
        let mut mgr = hw.lock().unwrap();
        let sys = HardwareManager::get_system_info();
        let metrics = mgr.read_metrics();

        let json_body = serde_json::json!({
            "system": sys,
            "metrics": metrics,
        });

        if let Ok(body_bytes) = serde_json::to_vec(&json_body) {
            send_response(&mut stream, 200, "application/json", &body_bytes);
        }
    } else if method == "POST" && path == "/api/mode" {
        if let Ok(json) = serde_json::from_str::<Value>(&body) {
            if let Some(mode) = json["mode"].as_str() {
                let mut mgr = hw.lock().unwrap();
                let _ = mgr.apply_profile(mode);
            }
        }
        send_response(&mut stream, 200, "application/json", b"{\"status\":\"ok\"}");
    } else if method == "POST" && path == "/api/fan" {
        if let Ok(json) = serde_json::from_str::<Value>(&body) {
            let mut mgr = hw.lock().unwrap();
            if let (Some(target), Some(mode)) = (json["target"].as_str(), json["mode"].as_str()) {
                let speed = json["speed"].as_u64().unwrap_or(50) as u8;
                let _ = mgr.apply_fan_control(target, mode, speed);
            } else if let (Some(cpu_mode), Some(gpu_mode)) = (json["cpu_mode"].as_str(), json["gpu_mode"].as_str()) {
                let cpu_spd = json["cpu_speed"].as_u64().unwrap_or(50) as u8;
                let gpu_spd = json["gpu_speed"].as_u64().unwrap_or(50) as u8;
                let _ = mgr.apply_fan_control("cpu", cpu_mode, cpu_spd);
                let _ = mgr.apply_fan_control("gpu", gpu_mode, gpu_spd);
            } else {
                let cpu = json["cpu"].as_u64().unwrap_or(0) as u8;
                let gpu = json["gpu"].as_u64().unwrap_or(0) as u8;
                let cpu_mode = if cpu == 100 { "max" } else if cpu == 0 { "auto" } else { "custom" };
                let gpu_mode = if gpu == 100 { "max" } else if gpu == 0 { "auto" } else { "custom" };
                let _ = mgr.apply_fan_control("cpu", cpu_mode, cpu);
                let _ = mgr.apply_fan_control("gpu", gpu_mode, gpu);
            }
        }
        send_response(&mut stream, 200, "application/json", b"{\"status\":\"ok\"}");
    } else if method == "POST" && path == "/api/battery" {
        if let Ok(json) = serde_json::from_str::<Value>(&body) {
            let limit = json["limit"].as_u64().unwrap_or(80);
            let mut mgr = hw.lock().unwrap();
            let _ = mgr.apply_battery_limiter(limit <= 80);
        }
        send_response(&mut stream, 200, "application/json", b"{\"status\":\"ok\"}");
    } else if method == "POST" && path == "/api/backlight" {
        if let Ok(json) = serde_json::from_str::<Value>(&body) {
            let timeout = json["timeout"].as_str().unwrap_or("on");
            let mut mgr = hw.lock().unwrap();
            let _ = mgr.apply_backlight_timeout(timeout == "on");
        }
        send_response(&mut stream, 200, "application/json", b"{\"status\":\"ok\"}");
    } else if method == "POST" && path == "/api/usb-charging" {
        if let Ok(json) = serde_json::from_str::<Value>(&body) {
            let enabled = json["enabled"].as_bool().unwrap_or(true);
            let mut mgr = hw.lock().unwrap();
            let _ = mgr.apply_usb_charging(enabled);
        }
        send_response(&mut stream, 200, "application/json", b"{\"status\":\"ok\"}");
    } else {
        send_response(&mut stream, 404, "text/plain", b"Not Found");
    }
}

fn send_response(stream: &mut TcpStream, status_code: u16, content_type: &str, body: &[u8]) {
    let status_line = match status_code {
        200 => "HTTP/1.1 200 OK",
        404 => "HTTP/1.1 404 Not Found",
        _ => "HTTP/1.1 500 Internal Server Error",
    };

    let response = format!(
        "{}\r\nContent-Type: {}; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: *\r\nConnection: close\r\n\r\n",
        status_line,
        content_type,
        body.len()
    );

    let _ = stream.write_all(response.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

