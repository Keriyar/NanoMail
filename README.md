# NanoMail

<div align="center">

![NanoMail Logo](assets/icons/NanoMail.ico)

**轻量级的 Windows Gmail 通知客户端**

灵感来源于 macOS 版 [Gmail Notification](https://github.com/crayonape/Gmail-Notification)

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Rust](https://img.shields.io/badge/Rust-2024-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%2F11-blue.svg)](https://www.microsoft.com/windows)

</div>

---

## ✨ 特性

- 🎨 **现代化 UI** - 采用 Slint 构建，磨砂玻璃效果、直角窗口，支持 Windows 原生半透明
- 🌓 **深色模式** - 完美适配日夜环境，支持主题手动切换与自动持久化
- 📧 **实时感知** - 融合后台低频轮询（10s）与窗口唤醒即时同步，秒级响应未读变化
- 🔐 **安全无忧** - OAuth2.0 授权机制，Token 使用 AES-256-GCM + 机器指纹加密存储
- 🖼️ **极速头像** - 智能头像缓存策略，减少网络请求，提升加载速度
- 🚀 **极致轻量** - 深度优化二进制体积（移除冗余 features），低内存占用
- 🎯 **托盘控制** - 单击显示/隐藏，右键快捷菜单，状态颜色指示

---

## 🚀 快速开始

### 系统要求

- **操作系统**: Windows 10 (1809+) 或 Windows 11
- **运行时**: 无需额外依赖（静态链接），开箱即用

### 下载安装

1. 从 [Releases](../../releases) 页面下载最新版本的 `nanomail.exe`
2. 双击运行，程序会自动在系统托盘启动（首次运行可能被杀毒软件误报，请添加信任）
3. 左键点击托盘图标或使用右键菜单打开主界面

### 首次使用

1. 点击主界面底部的 **➕ 登录/添加账户** 按钮
2. 浏览器会自动打开 Google 安全授权页面
3. 登录并授权 NanoMail 访问您的 Gmail（仅需只读权限）
4. 授权成功后自动返回，即刻同步未读邮件数和头像

---

## 🎯 功能说明

### 主界面交互
- **智能标题栏**：
  - `N` 状态灯：🟢 登录成功 / 🔴 登陆失败（可能是网络错误）
  - 未读邮件色块：🟢 获取未读邮件成功 / 🔴 获取未读邮件失败
  - 🌙/☀️ 主题切换：一键切换深色/浅色模式
  - ✉️ 快捷访问：直达 Gmail 网页版
- **账户列表**：
  - 实时显示各账户头像、昵称和精确的未读数
  - 账户状态独立显示，错误信息一目了然

### 系统托盘
- **左键单击**：快速显示/隐藏主窗口
- **右键菜单**：
  - **打开 Gmail**：打开默认浏览器的Gmail
  - **关于**：NanoMail的地址
  - **退出程序**：退出NanoMail

### 同步机制
采用高效的**混合驱动策略**：
1. **后台保活**：隐藏时每 10 秒极低功耗轮询，保持数据新鲜
2. **即时唤醒**：点击托盘图标显示窗口时，**立即触发**一次全量同步，确保所见即最新

---

## 🔧 开发指南

### 环境准备

1. **安装 Rust**:
   ```bash
   rustup default stable
   ```
2. **克隆仓库**:
   ```bash
   git clone https://github.com/yourusername/nanomail.git
   cd nanomail
   ```

### 构建命令

```bash
# 开发环境运行（自动加载 .env）
cargo run

# 发布版构建（极致体积优化）
cargo build --release
```

### OAuth2 配置
本项目依赖 Google Gmail API，开发前需配置凭据：
1. 前往 [Google Cloud Console](https://console.cloud.google.com/) 创建项目
2. 启用 **Gmail API**
3. 创建 **OAuth 2.0 Client ID** (Desktop App)
4. 设置环境变量（或此时运行会自动生成 `config.example.toml`）：
   - `GMAIL_CLIENT_ID`
   - `GMAIL_CLIENT_SECRET`

---

## 📁 项目结构

```
NanoMail/
├── src/
│   ├── main.rs              #应用入口：生命周期与事件循环
│   ├── config/              # 配置持久化与安全加密
│   ├── mail/                # Gmail API 客户端与 OAuth 逻辑
│   ├── sync/                # 异步同步引擎 (Tokio Select)
│   ├── tray/                # 系统托盘与原声菜单集成
│   └── utils/               # HTTP 连接池与工具链
├── ui/                      # Slint 声明式 UI 源码
│   ├── main.slint           # 主窗口布局
│   └── components/          # 按钮、列表项等可复用组件
├── assets/                  # 静态资源 (Icon/Font)
└── Cargo.toml               # 依赖管理与 Release Profile 优化
```

---

## 🛠️ 技术栈

| 核心组件 | 技术选型 | 亮点 |
| -------- | -------- | ---- |
| **语言** | Rust 2021 | 内存安全，零开销抽象 |
| **UI 框架** | Slint 1.8 | 轻量级，硬件加速，.60 语法 |
| **异步运行时** | Tokio | 高并发，非阻塞 I/O |
| **HTTP 栈** | Reqwest | 持久化连接池，TLS 支持 |
| **加密** | AES-GCM-256 | 本地数据高强度加密 |
| **构建优化** | LTO + Strip | Release 体积最小化 |

---

## 🔐 隐私与安全

- ✅ 使用 OAuth2.0 授权,**不存储密码**
- ✅ Access Token 和 Refresh Token 使用 **AES-GCM 加密**存储
- ✅ 加密密钥基于**机器指纹**派生,防止跨设备窃取
- ✅ 所有 API 调用使用 **HTTPS** 加密传输
- ✅ 账户数据存储在 `%APPDATA%/NanoMail/accounts/`(仅本地)
- ✅ **开源透明**,代码可审计

---

## 🤝 贡献指南

欢迎提交 Issue 和 Pull Request!

### 开发流程

1. Fork 本仓库
2. 创建特性分支: `git checkout -b feature/AmazingFeature`
3. 提交更改: `git commit -m 'feat: Add some AmazingFeature'`
4. 推送分支: `git push origin feature/AmazingFeature`
5. 提交 Pull Request

### 代码规范

- 遵循 Rust 官方代码风格(`cargo fmt`)
- 运行 Clippy 检查(`cargo clippy`)
- 添加必要的单元测试
- 更新相关文档

---

## 📄 许可证

本项目采用 [GPL-3.0 License](LICENSE) 开源协议。

**注意**: 本项目仅用于学术目的(毕业设计项目)。

---

## 🙏 致谢

- 灵感来源: [Gmail Notification (macOS)](https://github.com/crayonape/Gmail-Notification)
- UI 框架: [Slint](https://slint.dev/)
- 图标设计: [Icofront-阿里巴巴矢量图标库](https://www.iconfont.cn/)

---

## 📧 联系方式

- **Issues**: [GitHub Issues](../../issues)
- **Discussions**: [GitHub Discussions](../../discussions)

---

<div align="center">

**如果觉得这个项目有帮助,请给个 ⭐ Star 支持一下!**

Made with ❤️ by NanoMail Project

</div>
