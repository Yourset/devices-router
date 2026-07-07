use crate::app_state::AppRuntime;
use anyhow::{Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Deserialize)]
struct UpdateManifest {
    files: HashMap<String, UpdateFile>,
}

#[derive(Clone, Debug, Deserialize)]
struct UpdateFile {
    version: String,
    path: String,
    size: Option<u64>,
    sha256: Option<String>,
}

pub fn start_update_server(runtime: Arc<AppRuntime>, port: u16) -> Result<()> {
    let root = updates_dir();
    let listener = TcpListener::bind(("0.0.0.0", port)).context("bind update server")?;
    listener
        .set_nonblocking(true)
        .context("set update server nonblocking")?;
    runtime.log(format!("[更新] 已启动局域网更新服务：0.0.0.0:{port}\n"));
    while !runtime.should_stop() {
        match listener.accept() {
            Ok((stream, _)) => handle_update_request(stream, &root),
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(err) => return Err(err).context("accept update request"),
        }
    }
    Ok(())
}

pub fn check_remote_update(runtime: Arc<AppRuntime>, host: &str, port: u16) {
    let host = host.to_string();
    thread::spawn(move || {
        if let Err(err) = check_remote_update_inner(&runtime, &host, port) {
            runtime.log(format!("[更新] 检查更新失败：{err:#}\n"));
        }
    });
}

fn check_remote_update_inner(runtime: &Arc<AppRuntime>, host: &str, port: u16) -> Result<()> {
    let manifest_url = format!("http://{host}:{port}/manifest.json");
    let manifest_payload = http_get(&manifest_url).context("读取主电脑更新清单失败")?;
    let manifest: UpdateManifest =
        serde_json::from_slice(strip_utf8_bom(&manifest_payload)).context("解析更新清单失败")?;
    let Some(file) = manifest
        .files
        .get("desktop")
        .or_else(|| manifest.files.get("remote"))
        .cloned()
    else {
        runtime.log("[更新] 主电脑没有提供桌面客户端更新包\n");
        return Ok(());
    };
    if file.version == CURRENT_VERSION {
        runtime.log(format!("[更新] 已是最新版本：v{CURRENT_VERSION}\n"));
        return Ok(());
    }

    runtime.log(format!(
        "[更新] 发现新版本：v{CURRENT_VERSION} -> v{}\n",
        file.version
    ));
    let download_url = format!("http://{host}:{port}/{}", file.path.replace('\\', "/"));
    let payload = http_get(&download_url).context("下载更新包失败")?;
    verify_update_file(&file, &payload)?;
    let installer = std::env::temp_dir().join(format!("DevicesRouter-{}-setup.exe", file.version));
    fs::write(&installer, payload).context("写入更新安装包失败")?;
    runtime.log(format!("[更新] 已下载更新包：{}\n", installer.display()));
    launch_installer_and_exit(&installer)?;
    Ok(())
}

fn handle_update_request(mut stream: TcpStream, root: &Path) {
    let mut buf = [0_u8; 2048];
    let Ok(size) = stream.read(&mut buf) else {
        return;
    };
    let request = String::from_utf8_lossy(&buf[..size]);
    let Some(path) = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
    else {
        write_response(&mut stream, 400, "text/plain", b"bad request");
        return;
    };
    let requested = if path == "/" {
        "manifest.json"
    } else {
        path.trim_start_matches('/')
    };
    if requested.contains("..") || requested.contains(':') {
        write_response(&mut stream, 403, "text/plain", b"forbidden");
        return;
    }
    let file = root.join(requested.replace('/', std::path::MAIN_SEPARATOR_STR));
    match fs::read(&file) {
        Ok(payload) => {
            let content_type = if file.extension().and_then(|value| value.to_str()) == Some("json")
            {
                "application/json; charset=utf-8"
            } else {
                "application/octet-stream"
            };
            write_response(&mut stream, 200, content_type, &payload);
        }
        Err(_) => write_response(&mut stream, 404, "text/plain", b"not found"),
    }
}

fn write_response(stream: &mut TcpStream, status: u16, content_type: &str, body: &[u8]) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        _ => "Error",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
}

fn http_get(url: &str) -> Result<Vec<u8>> {
    let (host, port, path) = parse_http_url(url)?;
    let mut stream = TcpStream::connect((host.as_str(), port)).context("连接更新服务失败")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(8)))
        .context("设置更新读取超时失败")?;
    let request = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .context("发送更新请求失败")?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .context("读取更新响应失败")?;
    let Some(split) = response.windows(4).position(|value| value == b"\r\n\r\n") else {
        anyhow::bail!("更新响应格式异常");
    };
    let header = String::from_utf8_lossy(&response[..split]);
    if !header.starts_with("HTTP/1.1 200") && !header.starts_with("HTTP/1.0 200") {
        anyhow::bail!(
            "更新服务返回错误：{}",
            header.lines().next().unwrap_or("unknown")
        );
    }
    Ok(response[split + 4..].to_vec())
}

fn parse_http_url(url: &str) -> Result<(String, u16, String)> {
    let rest = url
        .strip_prefix("http://")
        .context("只支持 http 更新地址")?;
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let (host, port) = authority
        .rsplit_once(':')
        .map(|(host, port)| (host.to_string(), port.parse::<u16>()))
        .unwrap_or_else(|| (authority.to_string(), Ok(80)));
    Ok((host, port.context("更新端口无效")?, format!("/{path}")))
}

fn verify_update_file(file: &UpdateFile, payload: &[u8]) -> Result<()> {
    if let Some(size) = file.size {
        if payload.len() as u64 != size {
            anyhow::bail!("更新包大小不匹配");
        }
    }
    if let Some(expected) = file.sha256.as_ref().filter(|value| !value.is_empty()) {
        let actual = hex_lower(&Sha256::digest(payload));
        if !actual.eq_ignore_ascii_case(expected) {
            anyhow::bail!("更新包 sha256 不匹配");
        }
    }
    Ok(())
}

fn launch_installer_and_exit(path: &Path) -> Result<()> {
    Command::new(path)
        .arg("/S")
        .spawn()
        .context("启动更新安装器失败")?;
    std::process::exit(0);
}

fn updates_dir() -> PathBuf {
    app_dir().join("updates")
}

fn app_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn strip_utf8_bom(payload: &[u8]) -> &[u8] {
    payload.strip_prefix(b"\xef\xbb\xbf").unwrap_or(payload)
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn host_from_socket_addr(address: &SocketAddr) -> Option<String> {
    match address.ip() {
        IpAddr::V4(ip) => Some(ip.to_string()),
        IpAddr::V6(ip) if !ip.is_loopback() => Some(format!("[{ip}]")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_utf8_bom() {
        assert_eq!(strip_utf8_bom(b"\xef\xbb\xbf{}"), b"{}");
        assert_eq!(strip_utf8_bom(b"{}"), b"{}");
    }

    #[test]
    fn parses_http_url_with_port() {
        assert_eq!(
            parse_http_url("http://192.168.31.18:8767/manifest.json").unwrap(),
            (
                "192.168.31.18".to_string(),
                8767,
                "/manifest.json".to_string()
            )
        );
    }

    #[test]
    fn verifies_size_and_hash() {
        let payload = b"abc";
        let file = UpdateFile {
            version: "0.1.4".to_string(),
            path: "Devices Router_0.1.4_x64-setup.exe".to_string(),
            size: Some(3),
            sha256: Some(hex_lower(&Sha256::digest(payload))),
        };

        verify_update_file(&file, payload).unwrap();
    }
}
