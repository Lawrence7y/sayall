# 圆环标签对齐与应用短名（2026-10-12）

## 用户要求与当前行为

- 四方向文字以各自圆环扇区为中心，不再靠近或压住中心圆。采用 label 自身的 translate(-50%,-50%)：上/下位于 (50%,12.5%)/(50%,87.5%)，左/右位于 (12.5%,50%)/(87.5%,50%)；上下宽 86 px、左右宽 48 px，保留 11 px 低对比小字与两行上限。
- OpenApp 菜单标签去掉“打开”前缀，优先使用本产品应用库中的友好名称，再使用预设名称或文件名回退。未找到名称的注册应用显示“应用”，不显示 AUMID。
- 已知名称仅在菜单展示时缩写：DeepSeek Harness → DSH，性能监视器 → 性能监视，Visual Studio Code → VS Code，Edge/Chrome 浏览器 → Edge/Chrome，文件资源管理器 → 资源管理器；ChatGPT 保留常用名称。未知品牌保留原名，不盲删词或改应用路径。
- 真实应用库、启动目标、按键配置、打开/激活动作与设置页应用名称均不修改。不扫描第三方配置，不增加菜单线程上的文件/注册表查询。窗口尺寸、右下角位置、动画和取消规则沿用既有版本。

## 验证

- TDD 首次预设标签测试实际失败：原“打开微信”不等于新“微信”；新位置断言在旧 CSS 下失败。实现后通过。
- cargo test --workspace：502 passed / 14 ignored；前端全量 311 passed / 11 skipped；类型、Vite、fmt、workspace check 与 runtime-simulation check passed。
- 单元覆盖预设短名、应用库友好名、大小写扩展名、未知 AUMID、未知品牌，以及菜单选项执行时仍返回原始 OpenApp target、不修改应用库。
- Windows 原生探针使用生产 presentation_with_apps，假目标的完整名称实际转为 ChatGPT / DSH / 性能监视；不会执行应用动作。UI Automation 读回短名，左右文字水平中心分别与圆环扇区相差 1.5 / 0.5 物理像素（本机 150% 缩放），两侧基线一致。文字 UIA 边界含字体上/下沿，垂直允许半个 11 px em，不把它等同于 CSS 行盒中心；截图确认标签没有压住中心圈。
- 三次显示/复用/右下角锚定通过；动画帧不同，鼠标取消后隐藏，点击前后前台保持不变，迟到加载不复活。截图与证据：仓库外 verification/menu-label-final.png、menu-label-native-summary.log、menu-label-native-diagnostic.log 与 test-menu-labels.ps1。探针顶部的数字仅用于测试代次标记，不进入实际菜单标签。

## 本地安装包

- builds/compact-ring-labels/SayAll_0.8.6_compact-ring-labels_x64-setup.exe
- SHA-256：bf295dd1951ebfb60fe60eb655a1497b21251001fb5d8ceb6fcc244928b3fb7e
- 未签名本地测试包，未发布。28 个前端资源嵌入检查通过。
- 已正常退出、备份并覆盖安装原目录；主程序与构建产物仅有预期 3 字节安装补丁差异，helper/gadget 一致，settings.json 与 button-mappings.json 哈希不变。新版本启动、Vue、首次 IPC 和设备订阅正常。
- 安装证据：verification/menu-label-install-result.log、menu-label-package.log、menu-label-embedded.log。

实体双型号、特殊多屏/缩放与冷态按键完整验收仍 deferred；原取消上游迟到边界不因标签调整而宣称解决。按既有用户要求不提交、不发布，保留已有未提交工作。
