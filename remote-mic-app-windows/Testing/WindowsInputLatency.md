# 通信、普通按键与四向菜单响应优化

## 范围与预期

- GATT/普通按键/菜单日志提交不再执行文件 open、metadata、write 与轮转；有界后台 FIFO 负责写盘。磁盘卡住和队列满不得卡住输入，溢出聚合留痕；退出与受控日志读取有界 flush。
- 四向菜单预加载并复用同一隐藏窗口。每次新页面显示完成后才能标记 ready；旧代次、取消、页面未就绪不得执行选项或复活窗口。隐藏失败销毁，后续请求可重建。
- 不修改 BLE 参数、双击 300 ms、长按 550 ms、连发起始 350 ms、语音和弦间隔；单击无双击/长按配置时原本就在 DOWN 沿执行，继续保持。四向菜单方向原本直接执行，优化的是窗口准备与输入线程阻塞。

## 已执行验证（Windows x64，2026-10-12）

- TDD：队列测试最初因 DiagnosticWriter 缺失编译失败，实现后 8 个日志相关测试通过。慢后端阻塞时入队立即返回；容量满返回 false 并记录 dropped；FIFO、每条路径、写入失败、恢复与 flush 超时均覆盖。
- 真磁盘对照 200 条：同步追加 140136 us，后台队列提交 145 us；flush 后实际读回 400 行（同步与异步各 200）。这是一次本机应用线程开销样本，非延迟分位数或实体遥控器端到端提升。证据：仓库外 `verification/latency-log-tests.log`。
- `cargo test --workspace`：495 passed / 14 ignored。初次全量发现 updater 单测仍按同步写盘读文件，更新为 flush 后读回，最终通过。证据：`verification/latency-workspace-tests.log`。
- `cargo check -p sayall-windows-app --features runtime-simulation`、宿主 check、fmt、Vue 类型检查与 Vite build 通过；前端 305 passed / 11 skipped。保留既有 warning，不顺带修改。
- 最终生产菜单探针立即发出首次请求，日志实际出现 waiting_for_preload；首次显示 192 ms（含初始 renderer 加载），随后复用 40 / 52 ms。同一 HWND、3 次 ready / visible / focus_preserved / reused 为 true；每次关闭隐藏，加载前取消后仍隐藏。UI Automation 读回三个代次各自的新名称，未执行快捷键或触碰设备。证据：`verification/latency-menu-native-final.log`、`verification/latency-menu-diagnostic-final.log` 与 `verification/test-latency-menu.ps1`。之前一次窗口测试未 ready，补充生命周期日志与初始加载等待后完成上述最终复验，不采用该次失败作为成功证据。

## 第二轮：非阻塞回声诊断（后随菜单残留修复包安装）

- `button_mapping.rs` 删除注入后的同步 2 ms 轮询（最长等待 60 ms）。立即读到回声时直接记录；未读到且钩子存活时，将只读诊断挂到现有输入线程的 deadline 中，到期只读一次。诊断不作为动作成功或失败判据，不补发按键、不修改 DOWN/UP。
- 保留 60 ms 观察预算，但不阻塞后续输入。最多 64 条短生命周期条目（本机条目数据总计最多 2048 B，不含容器/临时日志等开销）；满时仅跳过诊断并留痕，动作仍已入队。退出取消尚未完成的检查；没有待处理检查/手势/菜单时仍使用 `recv()` 无限阻塞。不新增线程、周期定时器或预热活动；有待检查时最多为每条安排一次到期唤醒。待检查数据存在少量有界内存开销，不能据此宣称整机内存绝不增加。
- 新日志用 `operation_id` 关联 pending 与终态，明确 `scope=global_hook_window`。宿主计数包含其他程序与后续动作，延迟读数不能冒充本次注入的精确 ACK，更不能证明前台收到了输入。
- TDD：先新增 deadline / bounded FIFO / shutdown 用例，因 `EchoDiagnostics` 缺失编译失败（`verification/echo-red.log`）；实现后补充实际输入引擎测试，诊断挂起期间连续两组 DOWN/UP 仍按顺序交付、释放状态为空、退出清空诊断。
- 增加无回声受控耗时对照：以 `u64::MAX` 作为不可能达到的回声计数起点，不注入实际键；同一测试进程里复刻旧同步等待并调用新生产提交函数。本机单次样本旧等待 62106 us / 27 次读取，新提交 39 us，诊断仍在队列未等待到期。这是诊断提交成本，不是遥控器、输入法或整机资源 A/B；耗时仅输出，不用不稳定的墙钟阈值作为测试判据。
- Windows x64 自动化 passed：`cargo test --workspace -- --nocapture` 506 passed / 14 ignored（含新增 4 项）；`cargo fmt --all -- --check`、`cargo check --workspace`、`cargo check -p sayall-windows-app --features runtime-simulation` 通过。证据：仓库外 `verification/echo-workspace.log`、`echo-fmt.log`、`echo-check.log`、`echo-simulation-check.log`。
- 第二轮最初仅改变 Rust 输入引擎，未调整语音、手势阈值、蓝牙参数、菜单动画或前端。当时未构建/安装；后随用户要求修复菜单残留的本地包完成构建和原目录安装，配置保留与启动检查通过，见 `Testing/WindowsMenuWindowLifecycle.md`。整机 CPU/内存与双型号实体按键验收仍 deferred，未提交或发布。
- deferred：整机闲置 CPU / 工作集 / 唤醒次数 A/B、双型号实体按键事件到动作的 P50/P95 与成功率、第三方输入法冷态首用、睡眠恢复。自动化证明诊断不等待输入线程，不证明实体遥控器快了固定 60 ms。
- 慢动作检查已确认 `app_launcher::activate_or_launch` 有窗口激活复核等待。暂不直接异步化：后续快捷键可能在目标激活前注入旧窗口，需要另行设计有界队列、代次取消与目标顺序保障后再实现；缓存与动画也未在本轮改动。

## 本地测试包（上一轮，不含第二轮改动）

- `D:/Project/sayall/builds/latency-optimized/SayAll_0.8.6_latency-optimized_x64-setup.exe`
- SHA-256：`d6334de5f70093b42ff67e996772333f52442f44dddcc1b0fd5554560cf90415`
- 仅 Windows x64 本地测试；无 Authenticode / updater 签名，未发布。stage-bundle-inputs、NSIS 构建完成；26 个前端资源嵌入校验通过。证据：`verification/latency-stage.log`、`verification/latency-package.log`、`verification/latency-embedded.log`。
- 构建完成时未安装；随后用户明确要求安装进行测试，已在原安装目录覆盖升级。先备份旧载荷与用户配置并通过 Local\\SayAll-GracefulExit 正常退出，未强杀。安装后 app 与构建产物仅有预期 3 字节 NSIS 补丁差异，helper / gadget 哈希一致，三项优化标记存在，settings.json / button-mappings.json 哈希保持不变。安装版启动后存活，tauri_setup 与 Vue mount 完成；日志观察到设备服务发现、音频/控制订阅完成和一次菜单显示。证据：仓库外 `verification/latency-install-result.log`。完整双型号实体按键及睡眠恢复测试仍由用户进行，不扩大为已通过。

## 实体遥控器验收（deferred）

RC001 / RC003 分别执行并记录事件到动作耗时及成功率：

1. 闲置后第一次普通按键与第一次语音（不得只验证连续热态）。
2. 四个方向单击、快速连按、按住连发；含有/没有双击和长按映射。
3. 中心长按菜单，四方向每次执行一次；中心/返回取消、10 秒超时、配置修改与暂挂关闭。
4. 菜单还未显示时按方向、快速关开及取消，确认不执行错误选项或复活。
5. 断连、睡眠唤醒、退出：DOWN/UP 配对与恢复不回归；日志写盘失败不影响设备处理。
6. 执行动作可能包含应用启动、DDC/CI 亮度等外部耗时，应分别计量，不能把窗口显示样本当作这些功能最终完成。

上述实体链路、第三方输入法冷态首用和蓝牙无线吞吐未在本轮自动化/窗口探针中验收。当前不 commit、不发布，保留已有未提交工作。
