# 紧凑圆环菜单：右下角与出场动画

## 当前设计（2026-10-12 用户要求）

后续标签对齐与应用短名修订已安装，见 `Testing/WindowsRingMenuLabels.md`。下述初版验证与安装包作为历史证据保留；当前文字位置和最新安装包以下游修订为准。

- 从已有 public/RC003-remote-photo@2x.png 裁出真实方向圆环，不生成虚构遥控器，不保留遥控器其它部分。scripts/crop-remote-ring.py 可重复生成 158×158 RGBA 图片，四角透明，保留黑色圆环与中心。
- 原 500×340 的深色矩形面板改为 232×244 逻辑像素的透明、无边框、无原生窗口阴影载体；圆环直径 208 逻辑像素。仅保留照片、四方向功能标注和中心轻量取消入口。
- 功能文字为 11 px 低对比灰白字，最多两行，避免大标题/卡片/醒目箭头；完整名称仍通过 textContent/title 安全设置。中心 × 与取消字样进一步降低对比。
- 固定主显示器工作区右下角，距离右/下工作区边界各 12 逻辑像素；工作区排除任务栏，所以在时间上方、不盖住任务栏。每次打开重读工作区与缩放，避免普通分辨率变化后缓存位置过期。
- 本地页面开始透明，宿主显示后才添加 is-visible，播放 480 ms 上移 22 px + 淡入/轻微缩放动画，不会在隐藏预加载时把动画播完。不等待动画结束才允许遥控器选择，原手势阈值与按键时序不变。遵循系统 prefers-reduced-motion 设置。
- 原鼠标取消、中心/返回取消、方向选择、代次与超时守卫不变。

## 已执行验证

- 新位置测试先因 clock_anchor 缺失失败，实现后通过；覆盖 100%/150% 缩放、负坐标工作区与小工作区边界。新前端结构/动画测试先失败，实现后通过；图片 PNG 尺寸与 RGBA 类型通过。
- cargo test --workspace：499 passed / 14 ignored。前端全量：310 passed / 11 skipped。类型、Vite build、fmt、workspace check 与 runtime-simulation check 通过。保留既有 warning。
- 最终 Windows 原生探针（本机 150% 缩放）：三个代次均 ready/visible/reused/anchored_above_clock=true；读回窗口物理尺寸与实际工作区坐标符合 232×244 逻辑尺寸和右下角公式。UI Automation 观察到三个代次更新的标签。
- 生产 CSS 动画通过测试探针的 Web Animations API 暂停在 80 ms / 480 ms 两个时刻，保存同一 348×366 物理窗口的早/晚渲染帧。实际图片确认早帧更透明、位置更低，末帧完整上移；原生窗口外围没有不透明矩形背景。不是仅凭 API 返回值断言动画通过。
- 最终探针鼠标按下取消请求到达、窗口隐藏、加载前取消不复活；前台进程未变为菜单进程，点击前后前台 HWND 相同。运行期间用户切换其他窗口会使部分“与 setup 初始前台相同”字段为 false，该字段不扩大为菜单抢焦点结论。
- 窗口测试早期曾误取到同进程 22×22 辅助窗口，已改用原生 EnumWindows / GetWindowRect 找真实窗口截帧。鼠标验证在实际可见的出场动画结束后点击，不把完全透明的首帧当作鼠标命中目标。最终证据为仓库外 verification/menu-ring-native-summary.log、menu-ring-native-diagnostic.log、menu-ring-early.png、menu-ring-final.png 与 test-menu-ring.ps1。

## 本地测试包与安装

- builds/compact-ring-menu/SayAll_0.8.6_compact-ring-menu_x64-setup.exe
- SHA-256：8c1d009bf919812e130848688535b93e039f76e646e48db8e523844c8db83953
- 无 Authenticode/updater 签名，仅供本地测试，未发布。staging / NSIS 构建、28 个前端资源嵌入检查通过。
- 已通过正常退出路径关闭旧版并备份，覆盖原安装目录后核对：app 只有预期 3 字节 NSIS 差异，helper/gadget 哈希一致，settings.json 与 button-mappings.json 保留，三项圆环/动画标记存在。
- 新安装版已启动并存活；setup、Vue、首次 IPC 完成，设备服务发现与音频/控制订阅完成。证据 verification/menu-ring-install-result.log、menu-ring-package.log、menu-ring-embedded.log。

## 用户复测 / deferred

- 长按中心，确认实际功能名称在四方向上可读、大小合适，位置位于时间上方，出场缓缓上移。
- 四方向仍执行一次；再次中心/返回及鼠标点击取消仍正确；快速关开不复活旧动画或菜单。
- 实体 RC001/RC003 两型号、不同 DPI 的真实多屏/主显示器切换、任务栏特殊布局与睡眠恢复完整验收仍 deferred。原上游按键迟到问题仍参见 Bugs/2026-10-12-four-way-menu-cancel.md，不能由本次视觉修改宣称已解决。

保持既有未提交工作；按既有用户要求不 commit、不发布。
