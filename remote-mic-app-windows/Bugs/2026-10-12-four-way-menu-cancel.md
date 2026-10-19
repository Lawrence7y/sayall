# 四向菜单鼠标取消缺失与中心释放沿残留

- 发现日期：2026-10-12
- 状态：已修复确认问题并安装测试包；实体返回/中心取消响应等待复测
- 影响范围：0.8.6 本地四向菜单及响应优化测试版，Windows x64；双型号实体取消验收尚未完成
- 现象：用户报告鼠标点击、返回键、中心键不能取消。
- 预期：点击菜单（包括中心区域）只取消；重新按下遥控器中心或返回只取消，不执行原单击/双击/长按。打开菜单的原长按释放不能补发单击。

## 已确认事实与未知边界

1. 鼠标：生产窗口设置 set_ignore_cursor_events(true)，HTML 没有取消入口，确实穿透到后面的应用。这是功能缺失，不是用户配置错误。
2. 中心释放：菜单打开后 release_all 清除手势状态，原本仍按住的中心键 UP 被重置后的识别器重新解释，可在双击窗口结束后补发单击；日志出现菜单选项执行之后的 Ok Single mouse_click。新测试验证消费该释放后不再补发。
3. 实体按键：原失败日志中，菜单 generation=2 从 06:06:34.945 显示就绪到 06:06:44.039 超时，期间只有中心 UP；中心/返回新 DOWN 在 06:06:48.553 才进入语义引擎。没有证据证明是菜单状态机拒绝了已收到的取消 DOWN；上游迟到原因尚未定位。不得把已确认的鼠标/释放沿修复扩大成两型号全部按键取消已通过。

## 修复

- public/quick-menu.html 与 quick-menu.js：明确点击取消入口，pointerdown 先 preventDefault 防止按钮默认聚焦，click 兼容辅助输入；同一页面只提交一次，不执行菜单选项。
- quick_menu_overlay.rs：接收鼠标但保持非激活窗口。拦截保留的 .invalid 导航作为取消信号，验证短生命周期 generation 与当前窗口/引擎状态；保留域名的任何请求均拒绝导航，不发网络请求。鼠标取消路径有 source=mouse / accepted 日志。
- button_mapping.rs：菜单手势重置后吞掉打开菜单的中心长按释放沿；返回/中心按下取消记录 reason=cancel_button source=remote。显示与取消仍由原引擎代次守卫处理。

## 验证

- TDD 新增释放沿测试最初编译失败，实现后通过；鼠标脚本缺失时新前端测试失败，加入脚本后通过。
- cargo test --workspace：498 passed / 14 ignored；前端全量 307 passed / 11 skipped，后续 pointerdown 更新的两项测试单独通过。类型、Vite、fmt、workspace check 与 runtime-simulation check passed。
- 最终 Windows 原生测试：三次 ready/visible/reused/focus_preserved 为 true，实际鼠标按下取消信号到达，click_focus_preserved=true，四次隐藏（含加载前取消）为 true。UI Automation 观察三代名称，额外原生前台检查 menu_became_foreground=false / click_foreground_stable=true。
- 前两轮窗口测试包含前台切换/鼠标取消时点问题，不能作为通过证据；最后使用 generation=900003 ready 确认后才点击，并额外读取点击前后前台窗口验证。证据为仓库外 verification/menu-cancel-native-summary.log、menu-cancel-native-diagnostic.log 与 test-menu-cancel.ps1。
- 修复安装包：builds/menu-cancel-fix/SayAll_0.8.6_menu-cancel-fix_x64-setup.exe；SHA-256 11821e87a71fe78c3bff69349a1b3f0edc6d99c550915e86dd219e62a08bac7f。仅本地测试，无 Authenticode/updater 签名，不发布。
- 已正常退出旧版、备份并覆盖安装；主程序只有预期 3 字节 NSIS 差异，helper/gadget 哈希一致，settings/button-mappings 配置保留，启动/页面/首次 IPC/设备订阅完成。27 个前端资源嵌入检查通过。证据 verification/menu-cancel-install-result.log、menu-cancel-embedded.log。
- deferred：实体 RC001/RC003 再次中心 DOWN、返回 DOWN 与上游迟到来源，冷态与睡眠恢复。用户继续进行测试。

隐私检查：正文不含设备身份、用户配置正文、语音内容、真实用户路径或凭据。保持既有未提交工作，不提交、不发布。
