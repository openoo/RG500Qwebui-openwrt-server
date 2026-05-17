# rg500q-webserver

`rg500q-webserver` 是 RG500Q WebUI 的 Rust 后端 OpenWrt 包。

## 安装内容

```text
/usr/bin/rg500q-webserver
/etc/init.d/rg500q-webserver
/etc/config/rg500q-webserver
/www/rg500q/*
/www/cgi-bin/at-ws-info
/www/cgi-bin/at-log-clear
```

## 默认连接

```bash
option connection_type 'SERIAL'
option serial_port '/dev/ttyUSB3'
option serial_baudrate '115200'
option websocket_port '8765'
```

## RG500Q 适配点

- USB 设备匹配 `2c7c:0800`
- 默认 AT 口 `/dev/ttyUSB3`
- 数据通道恢复使用 `AT+QNETDEVCTL`
- 信号/小区查询使用 `AT+QCSQ`、`AT+QENG="servingcell"`
- 频段锁定使用 `AT+QNWPREFCFG`
- 兼容部分旧 `AT^...` 命令并自动翻译

## 编译

```bash
make package/rg500q-webserver/compile
```
