# LuCI App for RG500Q WebUI

这是 RG500Q WebUI 的 LuCI Web 管理界面，提供直观的图形化配置和管理功能。

## ✨ 功能特性

### 1. 基础配置
- **连接管理**：配置网络 (IP/端口) 或串口 (TTY/波特率) 连接。
- **服务控制**：一键启用/禁用服务，查看实时运行状态。
- **安全设置**：配置 WebSocket 鉴权密钥，限制外网访问。

### 2. 高级网络 (Advanced Network)
- **拨号协议**：选择 IPv4, IPv6 或 IPv4v6 双栈。
- **网卡接口**：支持 `auto` 自动探测 5G 模组网卡，或手动指定。
- **IPv6 增强**：配置 RA Master 模式和前缀扩展。
- **定时锁频**：
  - **双模式**：独立配置“日间模式”和“夜间模式”。
  - **精细控制**：支持 LTE/NR 频段 (Bands)、频点 (ARFCN)、PCI 锁定。
  - **自动化**：定时自动执行，配合飞行模式切换确保生效。

### 3. 通知与日志 (Notifications & Logs)
- **多通道推送**：支持 10 种主流推送服务，**多选启用**：
  - 企业微信 (WeChat Work)
  - PushPlus
  - Server酱 (ServerChan)
  - PushDeer (支持自建)
  - 飞书 (Feishu)
  - 钉钉 (DingTalk)
  - Bark (iOS)
  - Telegram Bot
  - 通用 Webhook
  - 自定义脚本 (Shell/Python)
- **动态配置**：勾选对应的服务后，自动展开相应的配置项 (Token, Key, URL 等)。
- **实时日志**：内置 WebSocket 日志查看器，实时显示系统运行日志和 AT 交互日志。

### 4. AT 调试与终端
- **Web 终端**：内置 Xterm.js 终端，直接在浏览器中发送 AT 命令。
- **快捷指令**：预置常用 AT 命令按钮 (信号查询、基站信息、短信读取等)。

## 📸 界面预览

(此处可添加界面截图)

## 🔧 安装与使用

本插件通常作为 `rg500q-webserver` 的配套界面安装。

```bash
# 安装
opkg install luci-app-rg500q-webui

# 访问
# 登录 LuCI -> 服务 (Services) -> RG500Q WebUI
```

## 🛠 开发说明

- **前端架构**：基于 LuCI JS (Client-side rendering)。
- **配置文件**：`htdocs/luci-static/resources/view/rg500q-webui/config.js`
- **依赖**：`rg500q-webserver` (后端核心服务)。
