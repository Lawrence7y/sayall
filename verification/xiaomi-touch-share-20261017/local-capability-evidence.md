# 本机能力取证（2026-10-17，仓库外，含设备标识，勿入库/勿外传）

## Windows / 硬件
- OS: Microsoft Windows 11 专业版 10.0.26200 build 26200
- Wi-Fi: Intel(R) Wi-Fi 6E AX210 160MHz（当前 Disconnected，台式机主要走以太网）
  - `netsh wlan show drivers`: Hosted network supported = No；Wireless Display Supported = Yes (Graphics Driver Yes, Wi-Fi Driver Yes)
- Bluetooth: 英特尔(R) 无线 Bluetooth(R)（USB\VID_8087&PID_0032）
- **本机蓝牙适配器 MAC：F0:D4:15:05:9B:04**（= BluetoothAdapter.BluetoothAddress 264793676421892）

## WinRT 能力探测（Windows PowerShell 5.1 + WinRT，真实执行）
```
adapter_address=264793676421892
is_low_energy_supported=True
is_peripheral_role_supported=True
is_central_role_supported=True
is_advertisement_offload_supported=True
wifi_direct_publisher_created=true
wifi_direct_status=Started
```
结论：本机蓝牙支持 BLE 外设角色（可做 GATT Server/广播）；Windows Wi-Fi Direct 自主 GO 广播可启动（WiFiDirectAdvertisementPublisher.Start → Status=Started，3 秒后 Stop）。

## 已安装的小米 PC 端软件（非本项目安装，调研对象）
- 小米电脑管家 5.8.0.14（`C:\Program Files\MI\XiaomiPCManager\5.8.0.14`）
- 小米云服务 3.0.2
- Xiaomi MiMo 26.1014.140810
- 运行中进程：XiaomiPcHost.exe
- 运行中服务：
  - DistributedService（XiaoMi Distributed Service）→ DistributedService.exe
  - dist_service（XiaoMi Dist Service）→ dist_service.exe
  - handoff_service（XiaoMi Handoff Service）→ handoff\handoff_svc.exe
  - micont_service（XiaoMi Continuity Service）→ micont_service.exe
- 安装目录可见的关键组件（仅列文件名，未读取任何私有配置/数据）：
  - `micont_rtm.dll`、`micontinuity_sdk.dll`、`dist_handoff_sys_sdk.dll`
  - `lyra_dist_data_srv.dll` / `lyra_dist_file_srv.dll` / `lyra_dist_fs_service.dll` / `lyra_rpc.dll`
  - `MiSmartShareHandoff.exe` / `MiSmartShareDLL.dll` / `MiSmartShareClrWrapper.dll`
  - `MiDropTransfer.dll` / `MiDropShellExt.dll` / `MiDistributedFileServer.dll`
  - `MiSharedAuth.dll` / `libWinMiTrustService.dll` / `PcClipboard/` 目录
  - `MiService2_Setup.exe`（约 263 MB，官方互联组件安装包）

## 已下载的协议参考源码（仓库外，仅作调研）
- CatShare（MIT，commit 067348d，kmod-midori/CatShare）：Android 端互传联盟（MTA）接收端实现，含 BLE 广播/GATT、ECDH+AES-CTR 会话、Wi-Fi P2P GO、HTTPS/WSS 传输
- MiLinkNFC（MIT，commit 7f96a9c，XFY9326/MiLinkNFC）：小米 NFC 标签协议（Handoff/MiConnect/一碰妙享/内容流转）
- 目录：`D:\Project\sayall\verification\xiaomi-touch-share-20261017\`
