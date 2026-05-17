use crate::client::ATClient;
use crate::config::Config;
use crate::models::get_ndis_disconnect_tx;
use crate::network;
use crate::rg500q;
use log::{info, warn, error, debug};
use std::time::Duration;
use tokio::time::{sleep, interval};
use tokio::process::Command;
use anyhow::Result;

use tokio::fs;

/// IP 连接状态，参考 QModem modem_dial.sh 的 connection_status 四状态设计
#[derive(Debug, Clone, PartialEq)]
enum IpStatus {
    /// AT 命令响应异常（非预期内容）
    Unexpected,
    /// 有响应但无有效 IP
    NoIp,
    /// 仅 IPv4
    Ipv4Only(String),
    /// 仅 IPv6
    Ipv6Only(String),
    /// IPv4 + IPv6 双栈
    DualStack(String, String),
}

impl IpStatus {
    fn has_ip(&self) -> bool {
        !matches!(self, IpStatus::Unexpected | IpStatus::NoIp)
    }
}

enum ConnectionState {
    Disconnected,
    FullStackConfigured,
}

pub async fn start_monitor(config: Config, at_client: ATClient) {
    info!("Starting dial monitor with Disaster Recovery...");
    
    let mut state = ConnectionState::Disconnected;
    let mut ping_fail_count = 0u32;
    let mut unexpected_response_count = 0u32;

    // 订阅数据通道断开事件，断线时无需等待轮询立即响应
    let ndis_tx = get_ndis_disconnect_tx();
    let mut ndis_rx = ndis_tx.subscribe();

    // 10 秒轮询定时器
    let mut poll_timer = interval(Duration::from_secs(10));
    poll_timer.tick().await; // 消耗第一个立即触发的 tick

    loop {
        // 同时等待：轮询定时器 或 NDIS 断开事件（哪个先到处理哪个）
        let ndis_disconnected = tokio::select! {
            _ = poll_timer.tick() => {
                false // 正常轮询
            }
            result = ndis_rx.recv() => {
                match result {
                    Ok(()) => {
                        warn!("[DATA-CHANNEL] Disconnect event received! Triggering immediate recovery.");
                        true
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!("[DATA-CHANNEL] Missed {} disconnect events, treating as disconnected.", n);
                        true
                    }
                    Err(_) => false,
                }
            }
        };

        // NDIS 断开事件直接触发恢复，跳过 IP 检查
        if ndis_disconnected {
            // 检查用户是否手动关闭了自动拨号，若关闭则不进行灾难恢复
            if is_auto_dial_disabled(&at_client).await {
                debug!("[monitor] data channel disconnected but automatic recovery is disabled. Skipping recovery.");
                state = ConnectionState::Disconnected;
                ping_fail_count = 0;
                unexpected_response_count = 0;
                continue;
            }
            if !matches!(state, ConnectionState::Disconnected) {
                state = ConnectionState::Disconnected;
            }
            trigger_disaster_recovery(&config, &at_client).await;
            ping_fail_count = 0;
            unexpected_response_count = 0;
            continue;
        }

        // 常规轮询：检查 IP 状态
        match check_ip_status(&at_client).await {
            Ok(ip_status) => {
                match ip_status {
                    IpStatus::Unexpected => {
                        unexpected_response_count += 1;
                        warn!("AT+CGPADDR returned unexpected response. Count: {}/3", unexpected_response_count);
                        if unexpected_response_count >= 3 {
                            warn!("3 consecutive unexpected AT responses. Triggering disaster recovery.");
                            trigger_disaster_recovery(&config, &at_client).await;
                            unexpected_response_count = 0;
                            ping_fail_count = 0;
                            state = ConnectionState::Disconnected;
                        }
                    }

                    IpStatus::NoIp => {
                        unexpected_response_count = 0;
                        // 检查用户是否手动关闭了自动拨号，若关闭则不触发灾难恢复
                        if is_auto_dial_disabled(&at_client).await {
                            debug!("[monitor] No IP but automatic recovery is disabled. Skipping recovery.");
                            state = ConnectionState::Disconnected;
                            continue;
                        }
                        if !matches!(state, ConnectionState::Disconnected) {
                            warn!("Lost IP address. Resetting state and triggering disaster recovery.");
                            state = ConnectionState::Disconnected;
                        } else {
                            warn!("No IP address detected. Triggering disaster recovery.");
                        }
                        trigger_disaster_recovery(&config, &at_client).await;
                        ping_fail_count = 0;
                        state = ConnectionState::Disconnected;
                    }

                    ref status if status.has_ip() => {
                        unexpected_response_count = 0;
                        log_ip_status(status);

                        match state {
                            ConnectionState::Disconnected => {
                                info!("IP address detected. Starting network setup...");

                                debug!("Initializing modem URC reporting configs...");
                                let _ = at_client.send_command("AT+CNMI=2,1,0,2,0".to_string()).await;
                                let _ = at_client.send_command("AT+CMGF=0".to_string()).await;
                                let _ = at_client.send_command("AT+CLIP=1".to_string()).await;
                                // 手册：AT+CPMS 的 mem3（接收存储）掉电不保存，重启后重置
                                // mem1/mem2 上电后与上次 mem3 保持一致，因此三者都需重新下发
                                let sms_mem = &config.advanced_network_config.sms_storage;
                                let cpms_cmd = format!("AT+CPMS=\"{}\",\"{}\",\"{}\"", sms_mem, sms_mem, sms_mem);
                                debug!("Setting SMS storage to {} (AT+CPMS)...", sms_mem);
                                let _ = at_client.send_command(cpms_cmd).await;

                                let actual_ifname = detect_modem_ifname(&config.advanced_network_config.ifname).await;
                                debug!("Auto-detected 5G interface: {}", actual_ifname);

                                if let Err(e) = network::setup_ipv4_only(&config, &actual_ifname).await {
                                    error!("Failed to setup IPv4 network: {}", e);
                                } else {
                                    debug!("IPv4 setup done.");
                                }

                                let pdp_type = config.advanced_network_config.pdp_type.to_lowercase();
                                // ipv6_needed：配置了 v6 协议类型（ipv4v6 / ipv6）
                                // 注意：不依赖 ipv6_present（AT+CGPADDR 可能只返回数据 PDP 的 IPv4，
                                // IMS/IPv6 地址不一定出现在响应中），只要配置了就尝试注入
                                let ipv6_needed = pdp_type.contains("v6") || pdp_type.contains("ipv6");

                                if ipv6_needed {
                                    info!("IPv6 configured (pdp_type={}). Injecting IPv6 interface...", pdp_type);
                                    if let Err(e) = network::inject_ipv6_interface(&config, &actual_ifname).await {
                                        error!("Failed to inject IPv6 interface: {}", e);
                                    } else {
                                        debug!("IPv6 Injection Completed.");
                                    }
                                }

                                state = ConnectionState::FullStackConfigured;
                                ping_fail_count = 0;
                                info!("Network setup complete. Full stack active.");
                            }

                            ConnectionState::FullStackConfigured => {
                                if !check_router_network_status(&config).await {
                                    ping_fail_count += 1;
                                    warn!("Router-side network check failed. Count: {}/3", ping_fail_count);
                                    if ping_fail_count >= 3 {
                                        warn!("Continuous 3 router-side failures detected! Triggering disaster recovery.");
                                        trigger_disaster_recovery(&config, &at_client).await;
                                        ping_fail_count = 0;
                                        state = ConnectionState::Disconnected;
                                        continue;
                                    }
                                } else if ping_fail_count > 0 {
                                    debug!("Router-side network recovered. Resetting failure count.");
                                    ping_fail_count = 0;
                                }
                            }
                        }
                    }

                    _ => {}
                }
            }
            Err(e) => {
                warn!("Failed to check IP status: {}. Retrying later.", e);
            }
        }
    }
}

/// 打印当前 IP 状态到日志
fn log_ip_status(status: &IpStatus) {
    match status {
        IpStatus::Ipv4Only(v4) => debug!("Connection status: IPv4 only ({})", v4),
        IpStatus::Ipv6Only(v6) => debug!("Connection status: IPv6 only ({})", v6),
        IpStatus::DualStack(v4, v6) => debug!("Connection status: Dual Stack (v4={}, v6={})", v4, v6),
        _ => {}
    }
}

async fn check_router_network_status(_config: &Config) -> bool {
    // 1) 检查 wan_modem 接口状态（路由侧）
    let status_out = Command::new("ifstatus")
        .arg("wan_modem")
        .output()
        .await;

    let output = match status_out {
        Ok(o) if o.status.success() => o,
        _ => {
            warn!("ifstatus wan_modem failed.");
            return false;
        }
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = match serde_json::from_str(&text) {
        Ok(j) => j,
        Err(e) => {
            warn!("Failed to parse ifstatus wan_modem JSON: {}", e);
            return false;
        }
    };

    let up = v.get("up").and_then(|x| x.as_bool()).unwrap_or(false);
    if !up {
        warn!("wan_modem is down.");
        return false;
    }

    // 2) 需要有 IPv4 地址，且有默认路由（target=0.0.0.0）
    let has_ipv4 = v.get("ipv4-address")
        .and_then(|x| x.as_array())
        .map(|arr| !arr.is_empty())
        .unwrap_or(false);

    let has_default_route = v.get("route")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter().any(|r| {
                r.get("target")
                    .and_then(|t| t.as_str())
                    .map(|t| t == "0.0.0.0")
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    if has_ipv4 && has_default_route {
        debug!("Router-side network status OK (wan_modem up + IPv4 + default route).");
        true
    } else {
        warn!(
            "Router-side network status not ready: ipv4={}, default_route={}",
            has_ipv4,
            has_default_route
        );
        false
    }
}

/// 检查 IP 状态，返回精细的四状态枚举
/// 参考 QModem modem_dial.sh check_ip() 的 connection_status 设计
async fn check_ip_status(at_client: &ATClient) -> Result<IpStatus> {
    let response = at_client.send_command("AT+CGPADDR".to_string()).await?;

    let content = match response.data {
        Some(c) => c,
        None => {
            warn!("AT+CGPADDR returned no data.");
            return Ok(IpStatus::Unexpected);
        }
    };

    debug!("IP Check Response: {}", content);

    let mut found_v4: Option<String> = None;
    let mut found_v6: Option<String> = None;
    let mut has_cgpaddr_line = false;

    for line in content.lines() {
        let line = line.trim();
        if !line.starts_with("+CGPADDR:") {
            continue;
        }
        has_cgpaddr_line = true;

        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() < 2 {
            continue;
        }

        let segments: Vec<&str> = parts[1].split(',').collect();
        // segments[0] 是 PDP 索引，从 [1] 开始是 IP
        for segment in segments.iter().skip(1) {
            let clean_ip = segment.trim_matches(|c| c == '"' || c == ' ' || c == '\r' || c == '\n');

            if clean_ip.is_empty() || clean_ip == "0.0.0.0" || clean_ip == "::" {
                continue;
            }

            // RG500QM-CN 的 IPv6 地址以点分十进制格式返回（16个字节，共15个点）
            // 例如: "32.8.0.2.0.2.0.1.255.255.255.255.255.255.255.255"
            // 标准冒号格式: "2001:db8::1" 也兼容处理
            let dot_count = clean_ip.chars().filter(|&c| c == '.').count();
            let colon_count = clean_ip.chars().filter(|&c| c == ':').count();

            if colon_count >= 2 {
                // 标准 IPv6 冒号格式
                debug!("Detected IPv6 (colon fmt): {}", clean_ip);
                found_v6 = Some(clean_ip.to_string());
            } else if dot_count == 15 {
                // RG500QM-CN 点分十进制 IPv6 格式（16字节，15个点）
                // 验证所有段都是 0-255 的数字
                let all_valid = clean_ip.split('.').all(|s| s.parse::<u8>().is_ok());
                if all_valid {
                    debug!("Detected IPv6 (dotted-decimal fmt): {}", clean_ip);
                    found_v6 = Some(clean_ip.to_string());
                } else {
                    debug!("Detected IPv4: {}", clean_ip);
                    found_v4 = Some(clean_ip.to_string());
                }
            } else if clean_ip.contains('.') && dot_count == 3 {
                // 标准 IPv4 格式（x.x.x.x）
                debug!("Detected IPv4: {}", clean_ip);
                found_v4 = Some(clean_ip.to_string());
            }
        }
    }

    // 如果根本没有 +CGPADDR: 行，视为异常响应
    if !has_cgpaddr_line {
        warn!("AT+CGPADDR response contains no +CGPADDR line: {}", content.replace('\n', " ").replace('\r', " "));
        return Ok(IpStatus::Unexpected);
    }

    Ok(match (found_v4, found_v6) {
        (Some(v4), Some(v6)) => IpStatus::DualStack(v4, v6),
        (Some(v4), None)     => IpStatus::Ipv4Only(v4),
        (None,     Some(v6)) => IpStatus::Ipv6Only(v6),
        (None,     None)     => IpStatus::NoIp,
    })
}

/// RG500Q 专用拨号函数。
///
/// 设计原则：后端不主动修改或激活 PDP 上下文。
/// PDP 配置（AT+CGDCONT）和激活（AT+CGACT）由用户在前端完成，
/// 模组内部已保存的 PDP 数据会在 QNETDEVCTL 时自动使用。
/// 后端只负责建立 Quectel 数据通道（AT+QNETDEVCTL=1,1,1）。
async fn perform_dial(_config: &Config, at_client: &ATClient) -> Result<()> {
    info!("[dial] Establishing RG500Q data channel ({}).", rg500q::qnet_connect_command());
    let _ = at_client.send_command(rg500q::qnet_connect_command().to_string()).await;
    Ok(())
}

/// RG500Q 没有 MT5700 的 AT^SETAUTODIAL 语义。
/// 保持函数用于兼容旧恢复流程，当前总是允许恢复。
async fn is_auto_dial_disabled(at_client: &ATClient) -> bool {
    let _ = at_client.send_command(rg500q::qnet_status_command().to_string()).await;
    false
}

/// 精简灾难恢复入口（只保留最有效、最快路径）
///
/// 快速恢复步骤：
/// 1) 重建 Quectel 数据通道（AT+QNETDEVCTL=1,1,0 -> AT+QNETDEVCTL=1,1,1）
/// 2) 重启路由侧网卡（ip link down/up）
///
/// 不再执行 HVSST/CFUN/CGATT/模组复位等慢恢复流程。
async fn trigger_disaster_recovery(config: &Config, at_client: &ATClient) {
    warn!("[FAST-RECOVERY] Rebuilding RG500Q data channel and restarting interface...");

    if try_dial_and_bind(config, at_client).await {
        info!("[FAST-RECOVERY] Recovery succeeded.");
    } else {
        warn!("[FAST-RECOVERY] Recovery failed this round; will retry on next monitor cycle.");
    }
}

/// 执行拨号并绑定数据通道，返回 true 表示成功
async fn try_dial_and_bind(config: &Config, at_client: &ATClient) -> bool {
    // 断开旧数据连接后重新连接，QNETDEVCTL 的 OK 仅代表命令下发成功。
    let _ = at_client.send_command(rg500q::qnet_disconnect_command().to_string()).await;
    sleep(Duration::from_secs(2)).await;

    // perform_dial 只建立数据通道，PDP 由模组内部数据驱动
    if let Err(e) = perform_dial(config, at_client).await {
        warn!("[dial] perform_dial failed: {}", e);
    }

    if !wait_for_ip(at_client).await {
        warn!("[dial] Timed out waiting for IP.");
        return false;
    }
    info!("[dial] IP obtained. Refreshing RG500Q data channel...");
    let _ = at_client.send_command(rg500q::qnet_connect_command().to_string()).await;
    sleep(Duration::from_secs(5)).await;
    let actual_ifname = detect_modem_ifname(&config.advanced_network_config.ifname).await;
    let _ = Command::new("ip").args(&["link", "set", "dev", &actual_ifname, "down"]).status().await;
    sleep(Duration::from_secs(1)).await;
    let _ = Command::new("ip").args(&["link", "set", "dev", &actual_ifname, "up"]).status().await;
    info!("[dial] Interface {} restarted. Recovery complete.", actual_ifname);
    true
}

/// 等待有效 IP，参考 QModem 增加 120 秒超时熔断
/// 返回 true 表示成功获取 IP，false 表示超时
async fn wait_for_ip(at_client: &ATClient) -> bool {
    debug!("Waiting for valid IP address (timeout: 120s)...");
    let max_retries = 60u32; // 60 * 2s = 120s
    let mut retries = 0u32;

    loop {
        if retries >= max_retries {
            error!("Timed out (120s) waiting for valid IP address.");
            return false;
        }
        match check_ip_status(at_client).await {
            Ok(status) if status.has_ip() => {
                debug!("IP successfully obtained.");
                return true;
            }
            Ok(IpStatus::Unexpected) => {
                warn!("Unexpected AT response while waiting for IP (retry {}/{}).", retries + 1, max_retries);
            }
            Ok(_) => {
                debug!("No IP yet, retrying ({}/{})...", retries + 1, max_retries);
            }
            Err(e) => {
                warn!("Error checking IP status: {}", e);
            }
        }
        retries += 1;
        sleep(Duration::from_secs(2)).await;
    }
}

async fn detect_modem_ifname(configured: &str) -> String {
    if !configured.is_empty() && configured != "auto" {
        return configured.to_string();
    }
    if let Some(iface) = detect_modem_interface().await {
        return iface;
    }
    "wwan0".to_string()
}

/// 基于 QModem 原理的绝对精准探测法：直接读取 USB 设备的 Vendor ID (厂商代码)
async fn detect_modem_interface() -> Option<String> {
    let net_dir = "/sys/class/net";
    let Ok(mut entries) = fs::read_dir(net_dir).await else { return None; };

    // Quectel RG500Q 专用：只匹配 Quectel VID 2c7c
    let valid_vids = [rg500q::QUECTEL_USB_VID];

    while let Ok(Some(entry)) = entries.next_entry().await {
        let iface = entry.file_name().into_string().unwrap_or_default();
        if iface == "lo" || iface.starts_with("br-") || iface.starts_with("wl") || iface.starts_with("ra") {
            continue;
        }

        let vendor_path_direct = format!("{}/{}/device/idVendor", net_dir, iface);
        let vendor_path_parent = format!("{}/{}/device/../idVendor", net_dir, iface);

        let mut vid = match fs::read_to_string(&vendor_path_direct).await {
            Ok(v) => v,
            Err(_) => String::new(),
        };
        if vid.trim().is_empty() {
            if let Ok(v) = fs::read_to_string(&vendor_path_parent).await {
                vid = v;
            }
        }

        let vid = vid.trim().to_lowercase();
        if !vid.is_empty() && valid_vids.contains(&vid.as_str()) {
            info!("Hardware probing success! Found 5G modem: {} (Vendor ID: {})", iface, vid);
            return Some(iface);
        }
    }

    warn!("No valid 5G/4G USB modem interface found based on Vendor ID.");
    None
} 
