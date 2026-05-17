include $(TOPDIR)/rules.mk

PKG_NAME:=rg500q-webserver
PKG_VERSION:=0.1.0
PKG_RELEASE:=1

PKG_BUILD_DEPENDS:=rust/host
PKG_BUILD_DIR:=$(BUILD_DIR)/$(PKG_NAME)-$(PKG_VERSION)
PKG_BUILD_PARALLEL:=1

include $(INCLUDE_DIR)/package.mk
include $(TOPDIR)/feeds/packages/lang/rust/rust-package.mk

define Package/rg500q-webserver
  SECTION:=net
  CATEGORY:=Network
  TITLE:=AT Command WebSocket Server (Rust)
  # 必须引入 ca-bundle，否则使用 rustls 请求外部 https 时会因为找不到根证书报错
  DEPENDS:=$(RUST_ARCH_DEPENDS) +ca-bundle +kmod-usb-serial +kmod-usb-serial-option +kmod-usb-net-qmi-wwan +kmod-usb-net-cdc-ncm +kmod-usb-net-cdc-mbim
endef

define Package/rg500q-webserver/description
  Rust implementation of RG500Q WebUI for OpenWrt.
  Provides WebSocket interface for AT commands.
endef

define Build/Prepare
	mkdir -p $(PKG_BUILD_DIR)
	$(CP) ./src $(PKG_BUILD_DIR)/
	$(CP) ./files $(PKG_BUILD_DIR)/
	$(CP) ./Cargo.toml $(PKG_BUILD_DIR)/
	if [ -f ./Cargo.lock ]; then $(CP) ./Cargo.lock $(PKG_BUILD_DIR)/; fi
endef

define Package/rg500q-webserver/install
	$(INSTALL_DIR) $(1)/usr/bin
	$(INSTALL_BIN) $(PKG_INSTALL_DIR)/bin/rg500q-webserver $(1)/usr/bin/
	
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

$(eval $(call RustBinPackage,rg500q-webserver))
$(eval $(call BuildPackage,rg500q-webserver))
