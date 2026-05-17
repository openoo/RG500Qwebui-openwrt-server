include $(TOPDIR)/rules.mk

PKG_NAME:=rg500q-webserver
PKG_VERSION:=1.0.1
PKG_RELEASE:=0

include $(INCLUDE_DIR)/package.mk

define Package/rg500q-webserver
  SECTION:=net
  CATEGORY:=Network
  TITLE:=AT Command WebSocket Server (Rust)
  # 必须引入 ca-bundle，否则使用 rustls 请求外部 https 时会因为找不到根证书报错
  DEPENDS:=+ca-bundle +kmod-usb-serial +kmod-usb-serial-option +kmod-usb-net-qmi-wwan +kmod-usb-net-cdc-ncm +kmod-usb-net-cdc-mbim
endef

define Package/rg500q-webserver/description
  Rust implementation of RG500Q WebUI for OpenWrt.
  Provides WebSocket interface for AT commands.
endef

define Build/Compile
	[ -x ./files/usr/bin/rg500q-webserver ] || { echo "missing prebuilt ./files/usr/bin/rg500q-webserver"; exit 1; }
endef

define Package/rg500q-webserver/install
	$(INSTALL_DIR) $(1)/usr/bin
	$(INSTALL_BIN) ./files/usr/bin/rg500q-webserver $(1)/usr/bin/
	
	$(INSTALL_DIR) $(1)/etc/init.d
	$(INSTALL_BIN) ./files/etc/init.d/rg500q-webserver $(1)/etc/init.d/

	$(INSTALL_DIR) $(1)/etc/config
	$(INSTALL_CONF) ./files/etc/config/rg500q-webserver $(1)/etc/config/

	$(INSTALL_DIR) $(1)/www/rg500q
	$(CP) -r ./files/www/rg500q/* $(1)/www/rg500q/
	
	$(INSTALL_DIR) $(1)/www/cgi-bin
	$(INSTALL_BIN) ./files/www/cgi-bin/at-ws-info $(1)/www/cgi-bin/
	$(INSTALL_BIN) ./files/www/cgi-bin/at-log-clear $(1)/www/cgi-bin/
endef

$(eval $(call BuildPackage,rg500q-webserver))
