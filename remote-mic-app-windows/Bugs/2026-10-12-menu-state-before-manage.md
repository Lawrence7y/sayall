# 菜单接入导致安装版启动退出

## 事实与复现

四向菜单首个安装包的载荷、26 个前端资源、独立菜单窗口均已验证，但实际安装版启动退出。重定向安装版 stderr 后复现退出码 101，错误为 `state() called before manage() for sayall_windows_app::AppState`。诊断日志停在 `tauri_setup phase=started`，没有 setup completed / Vue mount。

首个失败包 SHA-256：`703bc3ec5fddf1a5e043568011cc5b0590c6eb22ef7d000186079e05940129d0`。编译、载荷对齐和独立窗口通过不能替代真实主程序启动。

## 根因与修复

setup 开头注册菜单回调时同步调用 `app.state::<AppState>()`，但 AppState 在 setup 后半段才由 `app.manage(...)` 注册。改为在 `create_platform()` 返回后直接克隆已初始化的 platform，注册 ready/cancel 回调；不依赖尚未注册的 AppState，也在 Raw Input 监听启动前完成回调接入。

## 回归判据

复验工作区与 runtime-simulation 编译检查；重新构建安装包，核对实际安装载荷，启动该安装目录的主程序，确认进程持续存活、tauri setup completed、Vue mount 与初始 IPC completed。原生菜单窗口与实体按键验收仍单独记录，不以模块测试扩大启动或硬件结论。结果见 `Testing/WindowsFourWayMenu.md` 与仓库外 `menu-startup*.log`。
