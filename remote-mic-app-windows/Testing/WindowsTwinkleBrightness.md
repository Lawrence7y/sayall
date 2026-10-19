# Twinkle Tray 全部显示器亮度

## 三个亮度动作的最终规则（2026-10-12 用户纠正）

亮度区域只提供“调亮显示器 / 调暗显示器 / 切换亮度控制目标”三个动作。未切换、重启或循环回全部时，两个亮度动作调节所有可控屏幕；选中单屏时只调整该 ID。旧测试版 `twinkle_brightness monitor=null` 也在同一 FIFO 中执行时解析当前目标，修正切换后仍执行 All 的问题；旧编号配置不静默改绑。根因日志与回归记录见 `Bugs/2026-10-12-brightness-all-ignores-selected-target.md`。

以下五动作界面及固定 All 语义是此前阶段的历史记录，以本段三个动作规则为当前实现。

最终三动作复验：前端 304 passed / 11 skipped，无未处理错误；Rust 在发布编译结束后串行复验 485 passed / 14 ignored；fmt、workspace check、runtime-simulation 编译检查和 NSIS 构建 passed。修复前新 Rust 用例 failed，修复后 passed。编译同时运行的 Rust 复验曾因既有 HID 用例固定等待 100ms 而 failed，未改其业务逻辑或断言，保留失败日志；前端补充测试结束自动卸载页面，消除轮询在测试环境销毁后仍触发的未处理错误。

最终包位于仓库外 `builds/twinkle-three-actions/SayAll_0.8.6_twinkle-three-actions_x64-setup.exe`，SHA-256 `9be77c97f06666533869236722302a44952b8f4a8161f582f20fe176cc95119a`。已安装到现有自定义目录，主程序预期 3 字节安装补丁差、helper/gadget 哈希、25 个前端资源和启动后的 Vue mount / 初始 IPC 均 passed。此包取代五动作测试包用于当前验收。用户日志确认旧包识别到两块可控显示器，但最终包笔记本 + S82 亮度分别读回、两型号遥控器仍 deferred；不修改用户已保存映射、不提交、不发布。

## 单屏与切换扩展（第二阶段历史记录）

新增 `twinkle_brightness_selected`（方向 + 步长）和 `twinkle_cycle_monitor`（无参数）；保留既有 `twinkle_brightness` 的全部/旧编号语义。切换与调节在同一有界 FIFO 内执行，调节到执行时才解析当前目标。默认/重启为全部；轮换“全部 → 每块 ddcci / wmi 可控显示器 → 全部”。只用官方 `--List` / `--MonitorID`，ID 不进日志，不访问私有协议。

列表采用 UTF-8、ANSI 清理、64KiB 限制与 5 秒 CLI 预算；重复/子串歧义 ID 拒绝，重排不改变已选 ID。没有可控屏幕或列表错误不改目标；调节时目标已失联则拒绝本次写入、回到全部并提示重试。同名屏幕增加序号。测试按钮等待预算扩展为 60 秒，覆盖排队的列表+调节组合。

目标提示是主屏上的临时非聚焦窗口：静态本地 HTML，名称通过 JSON + textContent 填入；加载前隐藏并使用暗色背景，加载完成后显示 1.8 秒，允许鼠标穿透。新目标销毁旧提示；迟到加载不显示过期目标；8 秒兜底销毁未加载完成的隐藏窗口。关闭按键页仍可提示，不重新打开主窗口。

本机证据（`verification/twinkle-poc/`，仓库外）：

- `selected-hardware.log`：实际产品模块轮换到单屏，独立 Windows API 确认亮度不变；单屏 ID 调亮生效；轮换回全部不改变亮度；混合队列按正确目标调节。初始亮度为用户测试后的 5，结束恢复并读回 5。
- `overlay-native.log` / `test-overlay.ps1`：生产提示模块的独立 Tauri 实例确认窗口可见、键盘焦点不变、自动销毁；UI Automation 确认 S82 文字。仅检查本程序提示，不改映射，不调亮度。
- `target-overlay.png`：提示本身的局部截图，确认中文标题与 S82 显示正常；不包含桌面其它区域。早期在页面加载前截到白色空窗，已通过“页面就绪后显示/计时”修正并复验。
- 原生独立 Tauri example 必须 `--release --features tauri/custom-protocol`，并嵌入 Common-Controls v6 manifest；漏掉这两项分别导致开发 URL 空白和 C0000139 入口点失败。这是 standalone probe 的启动前提，正常 Tauri CLI 包已自动满足。现场 `test-overlay.ps1` 用 Windows SDK mt.exe 给探针副本嵌入清单，不改变产品 exe。
- 两块可控屏幕、重排/失联/同名/歧义 ID 使用纯模块夹具验证，不冒充多屏真机验收。

实体按键验收时，把两个按键分别改成“调亮当前目标 / 调暗当前目标”，再用第三个按键“切换亮度控制目标”；原有“调亮所有显示器 / 调暗所有显示器”不会跟随切换。RC001、RC003 与多个可控显示器仍须分别确认。上一阶段全部亮度用户反馈“测试没有问题”，未给具体型号，不扩大为双型号验收。

## 2026-10-12 继续开发与复验

- 保留第二阶段已有实现，完成 Rust 格式整理。按用户指示，先提供本地测试包，测试完成前不提交、不发布。
- 当前代码前端 `pnpm test --maxWorkers=2` 为 304 passed / 11 skipped；`pnpm build`、`cargo test --workspace -- --test-threads=1`、`cargo fmt --all -- --check`、`cargo check --workspace` 与 runtime-simulation 编译检查 passed。
- 首轮默认并发前端运行出现两个异步用例失败及七个未处理错误；Rust 默认并发运行的 `usage_counters_record_deduped_presses` 失败。限制前端 worker、串行 Rust 测试后完整套件通过。保留两轮日志，不改业务逻辑或测试断言来规避失败；默认并发的稳定性未在本次解决。
- 本次单屏硬件复验 failed：独立读回探针调用 Twinkle Tray `--List` 超时，在基线读回前退出，未执行亮度写入。先前 `selected-hardware.log` 和 `overlay-native.log` 的 passed 记录保留，不能表述为本次重新通过。实体遥控器与多屏验收仍 deferred。
- 本轮完整日志位于仓库外 `verification/twinkle-poc/resume-*.log`，用于区分当前复验和此前实验。
- 本地 NSIS 测试包构建 passed，25 个前端资源已确认嵌入。安装到已有自定义目录后，主程序与构建产物仅有安装期预期的 3 字节差异，helper / gadget 哈希一致，`twinkle_target`、`twinkle_overlay`、`brightness-target.html` 标记存在，安装后的 25 个前端资源校验 passed。通用安装校验脚本假定 LOCALAPPDATA 下的直接子目录，未识别现有自定义位置而 failed；按卸载注册信息定位后执行了上述实际载荷校验，未修改脚本范围。
- 包路径为仓库外 `builds/twinkle-selected-brightness/SayAll_0.8.6_twinkle-selected_x64-setup.exe`，SHA-256 `df602bc8f7b53d69ce8fb315f85a96e929f908b2785172bddd99bc51a714ee73`，无 Authenticode / updater 签名。旁边 `build-metadata.json` 记录基线提交、未提交状态与功能源码哈希；内嵌 HEAD 不能代表未提交改动。

## 第一阶段实现范围

普通按键单击、双击、长按可选“调亮所有显示器 / 调暗所有显示器”，每次 1–100 个百分点，默认 5。保存的动作使用 `twinkle_brightness` + `monitor: null`，平台调用官方 CLI `--All --Offset=<signed step> --Overlay`。

首版不提供单屏选择或切换目标；保留原动作字段兼容，旧单屏配置不静默改绑，重新选择全部动作才改成 `monitor: null`。不参与语音键、不修改默认绑定，亮度动作不允许 hold repeat。

后台只有一个执行线程和容量 4 的待执行通道。请求过多明确拒绝，不静默丢弃；每个 CLI 子进程有 5 秒预算，测试命令后台等待完成最多 30 秒；退出取消待执行请求、终止本功能创建的子进程并回收，不终止 Twinkle Tray 主程序。不通过 shell 执行用户命令，日志不含个人路径或显示器身份。

## 自动化

```powershell
node node_modules/vitest/vitest.mjs run
node node_modules/vue-tsc/bin/vue-tsc.js --noEmit
node node_modules/vite/bin/vite.js build
cargo fmt --all -- --check
cargo test --workspace
cargo check --workspace
cargo check -p sayall-windows-app --features runtime-simulation
```

本机结果：前端 302 passed / 11 skipped，Rust 全工作区 477 passed / 14 ignored，TypeScript / Vite build / fmt / workspace check / runtime-simulation check 均 passed（有既有编译 warning）。安装包及物理遥控器验收不包含在这些结果内。

覆盖：全部动作及步长保存、单击/双击/长按槽位、没有显示器选择控件、旧单屏配置不静默扩张、测试提示与错误、步长非法值拒绝；Rust 覆盖 signed offset/All 参数、无效配置、序列化往返与禁止重复。

## 本机硬件证据

环境：Twinkle Tray 1.17.2.0，一个可控 DDC/CI 物理显示器（S82），一个 `none` 不可控列表条目。VCP 最大亮度 100，初始 11。证据保存在仓库外 `D:/Project/sayall/verification/twinkle-poc/`，没有归档显示器 ID 或用户配置。

1. 独立 Rust UTF-8 捕获 `--List`、ANSI 清理、字段解析通过；PowerShell 默认解码曾乱码，Rust 明确 UTF-8 不存在该问题。
2. 单次调亮/调暗经 Windows DDC/CI 读回确认 `11 -> 12 -> 11`。两轮 60 秒闲置后首次调用至硬件读回约 406–438ms。
3. 100ms 并发三次 +1：第一轮 `11 -> 13`（failed），第二轮 `11 -> 14`（passed）。不能把并发视作可靠或凭样本归因内部机制。
4. `--All` CLI-only 串行三轮，每轮三次 +1，均 `11 -> 14`，共 9 次请求，退出耗时各 339–455ms；每轮恢复 11 并读回确认。证据 `all-serial.log`。
5. 编译产品平台模块的 `twinkle_all_probe`，一次提交三个 +1 给真实生产后台任务，三个完成回调返回后由独立 Windows API 探针读回 `11 -> 14`；恢复 `--All --Set=11` 后读回 11。证据 `production-worker.log`。此步骤测试产品后台，而不只是探针直接调用 CLI。

生产探针需显式 `--exercise`，会改变亮度且自身不负责恢复。只应在记录所有屏幕初始值、能可靠恢复时使用。现场包装脚本 `verification/twinkle-poc/test-production-worker.ps1` 仅允许一个可控且可读的物理屏幕，finally 恢复并硬件确认，不能把统一 Set 恢复法用于各屏初始值不同的多屏配置。

“请求已发送”与“硬件已改变”分开。产品不读取第三方内部状态，测试按钮不能仅依据 CLI 退出报告硬件成功。

## 实体遥控器 / 安装包验收（deferred）

RC001 和 RC003 分别执行：

- 安装本地包、启动 Twinkle Tray，先在 Twinkle Tray 确认每块屏幕可以控制。
- 普通按键分别配置调亮/调暗，步长 1 或 5；测试按钮与物理按键均观察真实亮度变化。
- 保存、关闭界面、重启后映射保留；导入导出保留亮度动作和步长。
- 验证三种手势不影响语音键实时开始/结束，不触发键盘/音量等额外动作（受现有按键能力边界约束）。
- 冷态首用、快速连按、方向反转、队列满后的错误提示；不得因请求正常退出就判定无丢步。
- 多块可控屏幕设不同初始亮度，用相对 +5/-5 检查各屏分别变化；边界 0/100 不越界；不可控屏幕保持不变。
- Twinkle Tray 未运行、未安装、执行失败/超时都明确报错。
- 使用后断连、睡眠唤醒、屏幕重插再试；退出期间待执行任务不复活、不强杀 Twinkle Tray。
- Microsoft Store 版、自定义安装路径与多屏归一化设置兼容性另行验证；当前可执行定位仅覆盖默认安装路径和商店执行别名。

本机 CLI 与平台后台 passed 不能替代 RC001/RC003 物理按键、多屏、安装升级或正式发行验收。
