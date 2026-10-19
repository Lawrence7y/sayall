# 四向菜单窗口生命周期回归

## 预期

1. 完整启动并创建主窗口后，没有 quick-menu 窗口，也没有它的独立任务栏项。
2. 首次请求才创建 renderer，真实页面加载后按同代次显示，不抢前台焦点。
3. 普通取消、鼠标取消、菜单超时、断连与配置变更都走隐藏验证；精确 HWND 原生不可见，保持工具窗口样式。
4. 关闭时去掉旧页面 is-visible 类；旧加载/取消回调不能复活窗口或关闭更新一代的菜单。
5. 即使 Windows 可见性或 app-window 样式与框架设置不一致，关闭也执行原生修复与读回；失败时请求销毁，下一次可重建。
6. 不新增轮询、线程或全局高精度计时器，不修改语音、亮度、手势阈值或遥控器配置。

## 已执行验证

- Windows x64 现场只读检查：旧安装版引擎已报告取消，菜单 HWND 仍为 visible，带 app-window 样式、不带 tool-window。证明存在残留窗口；导致首次状态漂移的具体调用未知。
- TDD 原生探针：`src-tauri/examples/menu_lifecycle_probe.rs`。旧代码输出 startup menu_absent=false，打开时 native_state=(true,false)，取消后的“隐藏+工具窗口”综合检查 false；不把这条综合失败单独等同于隐藏 API 失败。证据：仓库外 `verification/menu-residue-red-native.log`。
- 修复后的同一探针：startup menu_absent=true，首次和复用均 ready=true/native_state=(true,true)，主动制造 visible/app-window 漂移后关闭均 native_state=(false,true)，cancel_before_load=true。证据：`verification/menu-residue-green-native.log` 与 `menu-residue-green-diagnostic.log`。
- 探针需显式附加 Common Controls manifest。初次进程加载失败不算红态产品回归证据；Tauri probe 的退出码也不能替代原生状态断言。
- 完整 Rust 自动化 507 passed / 14 ignored，fmt 与 runtime-simulation check passed。结果见 `verification/menu-residue-workspace.log`、`menu-residue-fmt.log`、`menu-residue-check.log`。新增纯函数测试覆盖去除 app-window、增加 tool-window/noactivate、保留其余样式与幂等。
- 最终生产模式原生测试：探针必须用 `--features tauri/custom-protocol` 构建，确保加载嵌入本地页面而非配置中的开发服务器。此前未开启该 feature 的探针出现错误页重复 Finished / 未收到鼠标取消，不采用其渲染或鼠标结果作为产品验收；同时增加同代次显示/ready 幂等保护。最终两个探针在生产模式重跑通过，含 WM_CLOSE、主动状态漂移、加载前取消、同 HWND 复用、实际鼠标取消与焦点保持。
- 最终窗口样本首次显示 363 ms，复用 32 / 38 ms；这是一次独立探针窗口样本，不是实体按键端到端结果。鼠标测试在 Per-Monitor V2 DPI 上下文运行，使用物理坐标，并在输入前用 WindowFromPoint 核对目标仍属于探针，避免点击其它应用。证据：`verification/menu-residue-final-native.log`、`menu-residue-mouse-native.log`、对应 diagnostic 日志与 `test-menu-residue.ps1`。

## 本地测试包与完整启动

- 本地 Windows x64 修复包：`D:/Project/sayall/builds/menu-residue-fixed/SayAll_0.8.6_menu-residue-fixed_x64-setup.exe`。
- SHA-256：`20c1de070547a6d0dd694599ee8485c7d178d7ba7f5680be61dff62bd5d17f1b`。无 Authenticode / updater 签名，未发布。首次标准构建因缺测试签名密钥在签名阶段失败；采用仓库外构建覆盖配置，仅关闭本地包 updater artifacts 后重新成功构建，不修改正式配置或密钥。
- 28 个前端资源嵌入校验 passed。包还包含此前已验证但尚未安装的普通快捷键非阻塞回声诊断；不修改亮度配置。
- 已在原安装目录覆盖升级。先备份程序；主程序通过命名事件正常退出，BLE 清理读回 passed；常驻 helper 不随主程序退出，改用既有 stop-marker 接口正常停止后才安装，未强杀。首次安装脚本误把常驻 helper 当作自动退出进程而中止，没有执行安装；随后修正脚本并完成正常停止与安装。
- 安装退出码 0，主程序仅有已知 NSIS 的 3 字节补丁差异，helper / Gadget 哈希一致；4 个配置 JSON 在升级前后哈希不变。安装版启动后存活，Tauri setup / Vue mount 完成；15 秒后的 Win32 枚举确认不存在四向菜单窗口。证据：`verification/menu-residue-install-result.log`、`menu-residue-embedded.log` 与 `menu-residue-package-sha.json`。旧程序和配置备份保存在仓库外 backups 目录。
- 本轮不提交、不发布。完整启动已实际通过，不等同于双型号实体取消/睡眠恢复验收。

## 实体遥控器验收（deferred）

RC001 / RC003 分别覆盖：启动无菜单、第一次中心长按呼出、中心/返回/鼠标取消、四方向执行一次、10 秒超时、快关快开、断连与睡眠恢复。原生窗口探针不代表实体按键验收。

## 亮度范围

本轮不开放亮度连发、不修改配置。替代方案仅作评估：通过 Twinkle Tray 用户可见热键页面绑定并实测重复触发；或另行增加微软公开 DDC/CI 与 WMI 亮度后端，复用有界单工作者、按住时才调节、松手取消未执行请求。两者都需核对目标选择和硬件实际亮度，不使用 Twinkle Tray 私有 IPC/配置。
