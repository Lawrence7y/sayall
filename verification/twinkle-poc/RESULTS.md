# Twinkle Tray 小规模验证

## 范围

仅新增仓库外独立验证探针；没有改动 SayAll 生产源码、按键映射配置、安装包或第三方私有配置。使用 Twinkle Tray 官方 CLI，以及 Windows 公开 EnumDisplayMonitors / Physical Monitor / GetVCPFeatureAndVCPFeatureReply API。没有直接调用 Twinkle Tray 内部命名管道。

环境：Windows，本机安装并运行 Twinkle Tray 1.17.2.0，Rust 原生 Windows 编译。执行时间以 exercise*.log 文件时间为准。

## 1. 原生亮度控制：passed（CLI + 硬件状态读回）

列表含两个条目：一个 `none`，一个 `ddcci`（S82）。Windows API 独立识别到一个可读取亮度 VCP 0x10 的物理显示器，最大值 100，初始值 11。

两轮实验均观察到：

- `--MonitorID=<完整列表 ID> --Offset=1`：硬件亮度 11 -> 12。
- `--MonitorID=<完整列表 ID> --Offset=-1`：硬件亮度 12 -> 11。
- 每轮 finally 收尾使用 `--Set=11` 恢复，硬件读回确认 11；没有使用直接 DDC 写入兜底。

这验证 Twinkle Tray 官方 CLI 在本机确实改变硬件亮度，不仅是子进程退出成功。不等同于用户在 Twinkle Tray GUI 拖动滑块验收，也不等同于遥控器端到端验收。

## 2. Rust 捕获和列表解析：passed（当前安装版）

- 原生 Rust `Command` + stdout/stderr 捕获可取得 `--List` 输出。
- stdout 是合法 UTF-8，显式 UTF-8 解码避免 PowerShell 默认编码引入的中文乱码。
- 清理 ANSI 转义后可分块解析 MonitorID / Brightness / Type；保留 ID 仅在探针内存中，不写日志。
- 正式解析器应额外保留 Name 字段用于 UI，严格限制输出大小，验证字段、重复 ID 和支持类型；当前探针只是 PoC。
- 实际列表出现 `MonitorNum: 0` 的不可控条目，而文档的控制编号从 1 起。这意味着不能直接把列表编号当成稳定绑定契约。
- 正式功能应根据类型过滤不可控条目，优先用完整 MonitorID 绑定，不自动改绑失联目标。
- 两个解析单元测试通过（ANSI/Unicode/0 编号样本；缺字段/空输出拒绝）。
- 列表耗时：首次 PowerShell 1295 ms，Rust 初步读取 966 ms；两轮完整实验 790 / 638 ms。样本有限，不能称为性能分布。

## 3. 延迟和快速连按：部分 passed；并发稳定性 failed

| 实验 | 第一轮 | 第二轮 |
|---|---:|---:|
| 调亮 CLI 退出耗时 | 506 ms | 395 ms |
| 退出后等待硬件变化读回 | 217 ms | 55 ms |
| 调暗 CLI 退出耗时 | 405 ms | 339 ms |
| 退出后等待硬件变化读回 | 69 ms | 67 ms |
| 闲置 60 秒后首用 CLI 退出耗时 | 376 ms | 341 ms |
| 闲置后首用至硬件确认 | 438 ms | 406 ms |
| 3 次 +1，100ms 提交间隔的硬件结果 | 11 -> 13（failed） | 11 -> 14（passed） |

并发快速连按 2 轮中出现 1 次未完整累加，不能因第二轮通过而忽略第一轮失败。没有证据足以将原因锁定为特定内部节流或缓存机制；只确认对调用者而言结果不稳定。

第二轮追加“每次执行后确认硬件变化，再执行下一次”的串行实验：3 次 +1 均确认成功，11 -> 14；各步 390 / 373 / 375 ms，总计 1139 ms。此结果证明有确认门控的串行控制可行，**不能证明仅等待 CLI 子进程退出的现有队列足够可靠**。还需独立验证“仅串行等待退出”或“合并增量 + 延迟确认”的生产策略。

## 实施决策

1. 采用公开 CLI，不使用未承诺稳定的内部协议或第三方私有配置。
2. 列表获取是异步显式刷新操作，不能放入 BLE/输入钩子线程或高频 UI 轮询。
3. 首版只开放单次调亮/调暗；高频重复/长按连续控制不得直接每拍启动一个子进程。
4. 正式实施前，需用失败复现样本约束队列/合并策略，测量 CLI-only 串行模式，再决定限流、合并与确认机制。不能凭进程退出把动作标为硬件成功。
5. 商店版、自定义安装路径、多台可控显示器、不同亮度归一化设置、RC001/RC003 实体按键、睡眠/重插场景均 deferred。

## 文件与复现

- `probe.rs`：独立 Rust 探针，无第三方 Rust 依赖。
- `exercise.log`：第一轮，快速并发累加失败，但硬件恢复确认成功。
- `exercise-serial.log`：第二轮，并发通过；追加读回门控串行通过；恢复确认成功。
- `probe.exe` / `probe-tests.exe`：本机 Windows 编译产物，不进入产品分发。

在此目录运行 PowerShell：

```powershell
rustc --edition 2024 --test probe.rs -o probe-tests.exe
.\probe-tests.exe
rustc --edition 2024 probe.rs -o probe.exe
.\probe.exe                 # 只读列表和 DDC 硬件亮度
.\probe.exe --exercise      # 会短暂改变亮度；含 60 秒闲置与恢复
```

执行 exercise 前不要手动调节亮度，以免和恢复发生竞争。探针仅在一个 CLI ddcci 目标和一个可读取硬件目标、亮度远离边界时进行写入。恢复失败会明确报错，必须人工检查，不可宣称 passed。

## 需求收敛后的实施与验证

用户要求首版只控制所有显示器，不需要单屏选择；单屏选择与切换目标延期。

- 新增 `probe.exe --all-serial`：三轮、每轮三次 `--All --Offset=1 --Overlay`，仅串行等待 CLI 退出，不在每步之间读回门控；每轮硬件读回均 11 -> 14，随后恢复 11。9 次请求退出耗时 339–455ms。证据 `all-serial.log`。
- 新增生产后台探针 `crates/sayall-windows/examples/twinkle_all_probe.rs`；现场包装脚本 `test-production-worker.ps1` 一次提交三次真实后台请求，独立读回 11 -> 14，再恢复并读回 11。证据 `production-worker.log`。只在一个可控/可读屏幕的前置检查通过时执行，避免多屏初始值不同而恢复错误。
- UI 仅提供所有显示器调亮/调暗与步长；旧单屏配置保留，明确重选动作才扩展为全部，不因打开界面或修改步长静默扩展目标。禁用按住连发，保留有界串行后台。
- 修复现有接入的 TS 重复 mock 属性、Rust 日志摘要枚举遗漏和未声明 Tokio 依赖调用；使用 Tauri 已有 spawn_blocking + 有界 mpsc 等待完成，不新增运行时依赖。
- 前端全量 302 passed / 11 skipped；最后兼容性调整后按键页 71 passed / 1 skipped；Rust 全工作区 477 passed / 14 ignored；TypeScript、Vite、fmt、workspace check、runtime-simulation check 通过。
- 本地 NSIS 0.8.6 已生成，24 个前端资源嵌入校验通过。安装包：`D:/Project/sayall/builds/twinkle-all-brightness/SayAll_0.8.6_twinkle-all_x64-setup.exe`，SHA-256 `af113eb295197c03f1cbda8894292009e47bb1781ae564e06736bf11f8043a8f`。无 Authenticode / updater 签名，仅供本机测试，没有发布或安装。
- 原打包脚本因缺少本机 updater 私钥在签名阶段失败；使用仓库外 `unsigned-local.conf.json` 仅关闭此次构建的更新签名产物，正式配置未改变，重新构建成功。没有生成或读取新私钥。
- Git fetch 网络超时；Git 无配置作者身份，commit 被拒绝。相关 13 个文件已暂存，未冒用用户身份提交；基线 SHA `96a82b0872b6c4a64be0185fe310928e772655cd`，精确暂存补丁另存 `source.patch`，补丁哈希记录在安装包旁 `build-metadata.json`。保留了工作区既有修改与换行，不合入 main、不 push。
- RC001 / RC003 实体按键、安装升级、多个可控显示器、商店版、睡眠重插仍 deferred。

## 官方参考

- https://github.com/xanderfrangos/twinkle-tray#command-line-arguments
- https://github.com/xanderfrangos/twinkle-tray/wiki/Display-Detection-&-Support-Issues
- https://learn.microsoft.com/en-us/windows/win32/api/lowlevelmonitorconfigurationapi/nf-lowlevelmonitorconfigurationapi-getvcpfeatureandvcpfeaturereply
- https://learn.microsoft.com/en-us/windows/win32/api/physicalmonitorenumerationapi/nf-physicalmonitorenumerationapi-destroyphysicalmonitor
