# RG500Qwebui-openwrt-server

这是面向 Quectel RG500Q 系列 5G 模组的 OpenWrt WebUI 服务，基于 `mt5700webui-openwrt-server` 改造而来。当前默认适配你的 AW1000 环境：

- 模组：Quectel RG500Q-EA
- USB ID：`2c7c:0800`
- 默认 AT 串口：`/dev/ttyUSB3`
- 默认 WebSocket 端口：`8765`
- 默认 Web 面板路径：`/www/rg500q`

后端使用 Rust 实现 WebSocket AT 服务，LuCI 前端负责配置、日志、调试和内置模块管理页面。

## 功能

- WebSocket AT 命令转发
- 串口和网络 AT 两种连接方式，默认串口
- RG500Q 数据通道恢复，使用 `AT+QNETDEVCTL`
- 自动识别 Quectel 网卡，默认匹配 VID `2c7c`
- IPv4/IPv6 双栈 OpenWrt 接口配置
- 短信 PDU 读取、来电、新短信、存储占用通知
- PushPlus、Server 酱、PushDeer、飞书、钉钉、Bark、Telegram、通用 Webhook、自定义脚本通知
- 定时锁频，RG500Q 频段锁定使用 `AT+QNWPREFCFG`
- AT 调试终端和系统日志页面
- 兼容部分旧 MT5700 前端命令，会自动翻译为 RG500Q 命令，例如 `AT^MONSC` 到 `AT+QENG="servingcell"`

## 目录

```text
rg500q-webserver/          Rust 后端和 OpenWrt 包
luci-app-rg500q-webui/     LuCI 前端包
```

## 编译

把整个目录放到 OpenWrt SDK 的 `package/` 下，然后执行：

```bash
./scripts/feeds update -a
./scripts/feeds install -a
make package/rg500q-webserver/compile
make package/luci-app-rg500q-webui/compile
```

GitHub Actions 目前自动发布 LuCI 包和源码包。Rust 后端包需要完整 OpenWrt buildroot 或已缓存 Rust host 工具链的 SDK 环境；直接在 GitHub runner 的临时 SDK 中现编 `rust/host` 很慢，也容易失败。

## 安装

把生成的 `.ipk` 上传到路由器 `/tmp`，然后安装：

```bash
opkg install /tmp/rg500q-webserver_*.ipk
opkg install /tmp/luci-app-rg500q-webui_*.ipk
/etc/init.d/rg500q-webserver enable
/etc/init.d/rg500q-webserver restart
```

LuCI 菜单位置：

```text
Modem -> RG500Q WebUI
```

## 默认配置

```bash
config rg500q-webserver 'config'
    option enabled '1'
    option connection_type 'SERIAL'
    option serial_port '/dev/ttyUSB3'
    option serial_baudrate '115200'
    option websocket_port '8765'
    option pdp_type 'ipv4v6'
    option ifname 'auto'
```

如果你的 AT 口不是 `/dev/ttyUSB3`，可以在 LuCI 配置页面修改，也可以直接改 `/etc/config/rg500q-webserver`。

## RG500Q AT 指令适配

本项目按 Quectel RG50xQ/RM5xxQ 官方 AT 手册适配，核心命令包括：

- 模组信息：`ATI`
- 网络信息：`AT+QNWINFO`
- 信号质量：`AT+QCSQ`
- 小区信息：`AT+QENG="servingcell"`
- 数据通道连接：`AT+QNETDEVCTL=1,1,1`
- 数据通道断开：`AT+QNETDEVCTL=1,1,0`
- PDP/IP 状态：`AT+CGPADDR`
- LTE 频段：`AT+QNWPREFCFG="lte_band",...`
- NR 频段：`AT+QNWPREFCFG="nr5g_band",...`

频点/小区锁定走 `AT+QNWLOCK` 的最佳兼容实现。不同 RG500Q 固件对 `QNWLOCK` 的参数支持可能不同，如果某条锁频命令返回 `ERROR`，应以你手里的官方 PDF 和实际固件响应为准调整参数。

## 已在你的路由器上校验

通过 SSH/ubus-at-daemon 已确认：

- `/dev/ttyUSB3` 可正常执行 AT
- `ATI` 返回 `Quectel RG500Q-EA`
- 固件版本为 `RG500QEAAAR13A01M4G`
- `AT+QNWINFO`、`AT+QCAINFO`、`AT+QCSQ` 能返回响应
- 系统存在 `wwan0`/`wwan0_1` 和 Quectel USB VID `2c7c`

## 说明

`rg500q-webserver` 是新的 Rust 后端主程序；旧仓库里的 Python 后端未安装也已从本项目删除。内置 Web 页面会安装到 `/www/rg500q`，LuCI 页面通过 WebSocket 连接后端。
