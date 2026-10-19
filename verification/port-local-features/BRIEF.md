# 移植任务简报：旧版本地功能 → 1.0.0 + 手机一碰传

## 背景
- 工作区：`D:\Project\sayall\nfc-transfer`（git worktree，分支 `feat/nfc-phone-transfer`，dirty，正在合并，**不要** `git checkout/reset/stash`，**不要** commit）。
- 目标分支已经包含：1.0.0 主线全部代码 + 新功能「手机一碰传」（`crates/sayall-transfer`、`sayall-windows::phone_transfer`、`src-tauri::phone_transfer`、设置页区块等）。
- 来源工作区：`D:\Project\sayall\remote-mic-app-windows`（**只读，禁止修改**；HEAD=96a82b0，基于 0.8.6，工作区里有未提交的本地功能：Twinkle 亮度、四向快捷菜单、紧凑圆环菜单、日志/延迟优化等）。
- 已完成机械三方合并：来源工作区的 tracked 改动 + 24 个新文件已合入当前工作区；只剩 5 个文件有冲突标记（每文件 1 处，见 `D:\Project\sayall\verification\port-local-features\conflict-blocks.txt` 与 `merge-report.csv`）。

## 冲突文件与所属代理
- **Rust 代理**：
  1. `crates/sayall-windows/src/lib.rs`：保留我方内容 + 加入来源的 `pub mod quick_menu; pub mod twinkle_displays; pub mod twinkle_tray;`（按来源文件上下文补 `#[cfg(windows)]` 等属性，保持与来源一致）。
  2. `src-tauri/src/lib.rs`：**两边都要保留**——我方新增的 `phone_transfer::PhoneTransferController` 构造块 + 来源新增的 `quick_menu_overlay::register(...)` 块（顺序参考来源：先创建 platform，再 register 菜单回调，再接 phone_transfer 构造；注意 `saved_settings.phone_transfer` 字段来自我方）。
- **前端/文档代理**：
  3. `src/pages/ButtonsPage.test.ts`：合并 import（`afterEach, beforeEach, describe, expect, it, vi` + `enableAutoUnmount, flushPromises, mount`）。
  4. `ATTRIBUTION.md`：保留我方既有全部内容 + 来源新增的「菜单残留窗口修复与亮度替代方案评估/通信与按键实时路径优化/四向快捷菜单/Twinkle Tray…」等章节，去重、保持章节顺序合理。
  5. `TODO.md`：保留我方「手机一碰传」章节 + 来源新增的「通信与按键响应优化/四向快捷菜单/Twinkle Tray 显示器亮度/2026-10-10 本地性能优化」等条目，去重（来源侧末尾重复的「2026-10-10 本地性能优化」只保留一份）。
- 其余已合入文件不要回退；如合并结果有语义问题（重复函数、旧 API 调用），在保持两边功能的前提下修正。

## 关键边界（必须遵守）
- 不得删除/回退「手机一碰传」任何代码或文档；不得改动 `docs/decisions/0004-phone-transfer-lan-service.md`、`docs/investigations/`（Lead 正在写 C 调研）。
- 不得修改来源工作区 `D:\Project\sayall\remote-mic-app-windows`。
- 不要 commit、不要安装、不要重启正在运行的 SayAll；不要修改用户配置目录（`%APPDATA%\app.getsayall.remote-mic.windows`）。
- 仓库规则见 `AGENTS.md`（TDD、隐私日志、验证词汇）；只做本移植相关最小修改。
- 新模块/示例的目标是“合并后能编译通过且行为等价”，旧版 API 与 1.0.0 有差异时以 1.0.0 侧接口为准做适配。

## 验证要求
### Rust 代理（唯一允许运行 cargo 的代理）
- `cargo fmt --all` 后 `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets`（覆盖 examples；新 examples 若因 API 漂移失败需适配）
- `cargo test -p sayall-windows`（注意该 crate 测试多，允许较长耗时）
- `cargo test -p sayall-windows-app`
- `cargo check -p sayall-windows-app --features runtime-simulation`
- 若时间允许：`cargo test --workspace`
- 报告真实结果；失败的测试若与本移植无关（环境性），注明并给出证据。
### 前端/文档代理（不要运行 cargo；可运行 pnpm）
- `pnpm test`
- `pnpm build`（vue-tsc + vite）
- 新增的 `src/lib/quick-menu-*.test.ts` 必须全绿；ButtonsPage 既有测试不得回归。
- 文档冲突解决后检查：`ATTRIBUTION.md`、`TODO.md`、`docs/installation-and-configuration.md` 没有冲突标记、没有重复章节。

## 交付报告格式（最后一条消息）
- 改了哪些文件（按目录）；
- 命令 + 真实结果摘要（通过/失败数量）；
- 仍存在的问题/风险/需要 Lead 决策的点；
- 不要粘贴大段日志。
