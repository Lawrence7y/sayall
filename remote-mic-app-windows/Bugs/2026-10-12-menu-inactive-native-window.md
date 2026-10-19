# 四向菜单已关闭，但原生窗口与任务栏图标残留

- 发现日期：2026-10-12。
- 状态：已修复并在原目录安装本地测试包；Windows 原生/生产模式鼠标/启动与自动化通过，双型号实体验收待完成。见 `Testing/WindowsMenuWindowLifecycle.md`。
- 影响范围：本地 0.8.6 菜单窗口复用版本，Windows x64；与遥控器型号无关的窗口生命周期路径，双型号按键验收仍需分别执行。
- 现象：用户报告启动后出现四向菜单图标、不能关闭。现场日志显示中心键已取消菜单，但 Win32 枚举仍观察到可见的菜单窗口，带 `WS_EX_APPWINDOW`、不带 `WS_EX_TOOLWINDOW`；随后鼠标取消请求因旧菜单代次失效被拒绝。
- 正常预期：启动时不创建菜单窗口；只有中心长按请求才创建。关闭后不可见、无独立任务栏图标，重复呼出可复用，不抢前台焦点。
- 根因边界：原实现启动即预建菜单窗口，关闭只依赖框架 `hide()`，缺少原生可见性与样式读回。代次保护正确拒绝了过期请求，但不能修复残留原生窗口。导致现场窗口状态改变的最初外部/框架调用尚未知，不凭推测归因给 BLE、输入法或回声优化。
- 修复：`src-tauri/src/quick_menu_overlay.rs` 取消启动预建，改为首次请求创建；隐藏/显示在 UI 线程对精确 HWND 调用公开 Win32 API，读回 `IsWindowVisible` 和工具窗口样式，移除 `WS_EX_APPWINDOW`、增加 `WS_EX_TOOLWINDOW` / `WS_EX_NOACTIVATE`，保留其它样式。关闭清除旧页面动画可见类；隐藏验证失败才请求销毁。取消后的迟到页面事件与孤儿鼠标关闭只修复 inactive 窗口，不取消新的有效菜单。
- 失败路径：显示验证失败时立即隐藏/请求销毁并取消对应引擎代次。销毁提交只记录 accepted，不把 API 受理当作已销毁。没有新增周期检查、后台线程或快捷键补发。
- 受控实验：新增 `src-tauri/examples/menu_lifecycle_probe.rs`，包含主窗口创建顺序、首次显示、复用、通过公开 Win32 调用制造 visible/app-window 状态漂移、关闭读回及加载前取消。旧版原生检查 failed，新版所有检查 passed。探针最初缺 Common Controls manifest 无法启动，补 manifest 后才采用实验结果；探针运行时返回码不足以判定通过，脚本对输出的原生状态逐项断言。
- Windows 自动化：`cargo test --workspace` 507 passed / 14 ignored，fmt 与 runtime-simulation check passed。生产模式探针需用 `tauri/custom-protocol` feature；不把开发服务器错误页的 load/visible 当作菜单已渲染。实际鼠标取消、原生关闭、复用与焦点保持均 passed。安装前命名事件正常退出主程序，再通过既有 helper stop-marker 停止常驻助手，不强杀；4 个配置文件保留、完整启动无菜单 HWND。完整证据见验收文档。
- 限制：取消启动预加载降低启动驻留成本，但首次打开增加 renderer 创建成本；不能承诺冷态首次菜单更快。未知的外部窗口样式篡改与所有 Windows Shell 场景不能由一次探针穷尽。
- 隐私：记录仅包含功能、短生命周期代次和布尔状态，不含用户路径、窗口内容、设备身份或第三方私有配置。
