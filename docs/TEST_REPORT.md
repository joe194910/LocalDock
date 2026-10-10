# 实际测试报告

测试日期：2026-10-09

## 已执行

- `npm.cmd install`：通过。
- `npm.cmd run typecheck`：通过。
- `npm.cmd run build:web`：通过，生成 `dist/client`。
- `npm.cmd run build`：通过，生成前端与站点打包目录。
- `npm.cmd run test:sites`：通过，4 项测试全部成功。
- `npm.cmd run validate:packages`：通过，共验证 9 个声明式包。
- 中文乱码扫描：通过，未发现替换字符或历史乱码片段。
- `npm.cmd run tauri -- info`：检测到 WebView2 与 Visual Studio 生成工具。

## 环境阻塞

- 已尝试通过 Rustup 安装 stable-msvc。下载 `rustc 1.99.0` 时网络响应超时并回滚；随后执行 `npm.cmd run tauri:build` 时无法取得 Cargo 元数据，因此无法实际编译 Rust 后端、生成 `Cargo.lock` 或产出 NSIS EXE。
- 浏览器运行时没有可用会话，无法执行目标视口截图对照；未把静态代码检查冒充视觉验收通过。
- 当前环境未提供可授权的 1Panel V2 实例，PanelBackend 只实现了能力边界和明确阻塞，未冒充真实 API 验收。
- 当前环境没有用于安全测试的 MySQL、Redis、MinIO、Qdrant 凭据，因此只验证 TCP 可达性代码路径，未冒充协议鉴权通过。

## 发布前必须补测

- Windows 10 22H2、Windows 11、普通用户、PowerShell 5.1、中文与空格路径。
- DPAPI 秘密加密、四种协议只读探针、连接错误分类。
- 独立 Compose 安装/恢复/取消/并发冲突。
- 固定版本 1Panel API 授权、编排任务轮询与实例关联核验。
- JDK、Node.js、Maven、Git 的官方摘要下载、安全解包、环境变量回滚和新进程验证。
