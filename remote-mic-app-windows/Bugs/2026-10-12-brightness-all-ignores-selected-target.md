# 旧亮度动作忽略已切换显示器

## 触发与事实

上一测试版亮度界面有五个动作入口。用户配置旧的全部显示器调亮/调暗动作，再配置目标切换；切换到笔记本或 S82 后调节，用户观察到始终改变 S82。

2026-10-12 01:39 UTC 诊断日志显示切换由 `twinkle_target cycle=true` 执行；随后 Up / Down 双击分别执行 `twinkle_brightness direction=Up/Down steps=5 monitor=all`。这确认旧动作固定调用 All，未使用已切换目标。不能仅凭该日志推断笔记本硬件可控性。

## 修复

- 亮度界面只保留调亮、调暗、切换三个动作；未切换/重启时控制全部，切换后跟随选中的 ID。
- 旧 `twinkle_brightness monitor=null` 请求改为在串行任务执行时解析当前目标，不要求重新配置已保存按键。旧的明确编号配置保留目标，避免静默改绑。
- 日志把旧全部配置的执行目标标为 `monitor=current`，避免继续错误归因成 All。
- UI 回归断言亮度动作恰好三个，覆盖默认全部、切换、步长、三种手势、失败提示与旧编号配置。Rust 新回归用例先 failed，确认旧全部请求不能生成动态目标任务；修复后复验。

## 验证边界

前端、Rust 和本地包复验结果见 `Testing/WindowsTwinkleBrightness.md` 与仓库外 `verification/twinkle-poc/three-*.log`。笔记本 + S82 两屏实际分别读回、RC001/RC003 实体按键仍须由实际测试确认，不把 CLI 受理当硬件成功。按用户要求不提交、不发布。
