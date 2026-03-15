# GhostPrompt

> 轻量级桌面端 AI 实时字幕与面试辅助工具，基于 Tauri + Rust 构建。

在视频会议、在线面试等场景下，实时捕获系统音频，通过 STT + LLM 流式处理，以透明悬浮字幕的形式将结果叠加在桌面最上层，全程鼠标穿透，不干扰任何下层操作。

---

## 功能特性

- **幽灵悬浮窗**：无边框、完全透明、永远置顶，支持鼠标穿透（不遮挡会议软件操作）
- **双路音频捕获**：通过 WASAPI Loopback 捕获系统扬声器输出，可选同时采集麦克风
- **智能静音检测（VAD）**：基于 RMS 音量阈值自动断句，避免无效 API 请求
- **极速语音转文字（STT）**：对接 Deepgram API，低延迟返回转录文本
- **LLM 流式处理**：支持任意 OpenAI 兼容接口（OpenAI / DeepSeek / Ollama 等），打字机效果实时渲染
- **双模式切换**：
  - **翻译模式**：将外语实时翻译为中文
  - **面试模式**：提炼面试官问题要点，输出简洁的答题提示
- **自定义 System Prompt**：支持用户自定义提示词，覆盖默认模式提示词
- **全局快捷键**：`Ctrl+Shift+G` 一键切换幽灵模式（鼠标穿透开/关）
- **本地配置持久化**：所有配置（API Key、模式、提示词）保存在本地，无需后端服务

---

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | [Tauri 1.x](https://tauri.app/) |
| 后端逻辑 | Rust（cpal / reqwest / tokio） |
| 前端 UI | HTML + CSS + 原生 JavaScript |
| 语音识别 | [Deepgram API](https://deepgram.com/) |
| 大语言模型 | OpenAI 兼容接口（支持 GPT-4o / DeepSeek / Ollama 等） |
| 音频捕获 | cpal + WASAPI Loopback（Windows） |
| VAD | webrtc-vad |

---

## 快速开始

### 环境要求

- [Rust](https://www.rust-lang.org/tools/install) >= 1.70
- [Node.js](https://nodejs.org/) >= 18（Tauri CLI 依赖）
- Windows 10/11（当前版本主要在 Windows 上测试）

### 安装依赖 & 启动

```bash
# 安装 Tauri CLI
cargo install tauri-cli

# 进入项目目录
cd ghostprompt

# 启动开发模式
cargo tauri dev
```

### 构建发布包

```bash
cargo tauri build
```

---

## 配置说明

首次启动后，点击界面右上角设置图标，填写以下配置：

| 配置项 | 说明 |
|--------|------|
| Deepgram API Key | 用于语音转文字，申请地址：https://deepgram.com/ |
| LLM API Key | 大模型服务的 API Key |
| LLM Base URL | API 基础地址，默认 `https://api.openai.com/v1`，可替换为 DeepSeek / Ollama 等 |
| LLM Model | 模型名称，如 `gpt-4o`、`deepseek-chat`、`llama3` |
| 工作模式 | `translate`（翻译）或 `interview`（面试辅助） |
| System Prompt | 自定义提示词，留空则使用模式默认提示词 |

配置保存在本地用户目录下，无需联网同步。

---

## 快捷键

| 快捷键 | 功能 |
|--------|------|
| `Ctrl+Shift+G` | 切换幽灵模式（启用/禁用鼠标穿透） |

> 启用幽灵模式后，点击事件将穿透字幕窗口直达下方应用；禁用后可正常拖动窗口或操作设置面板。

---

## 项目结构

```
ghostprompt/
├── src/
│   └── index.html          # 前端 UI（悬浮窗 + 设置面板）
└── src-tauri/
    ├── src/
    │   ├── main.rs         # 应用入口、状态管理、IPC 命令注册
    │   ├── audio.rs        # 音频捕获、VAD 断句、捕获循环
    │   ├── stt.rs          # Deepgram STT 客户端
    │   ├── llm.rs          # OpenAI 兼容 LLM 流式客户端
    │   └── config.rs       # 本地配置读写、System Prompt 管理
    ├── Cargo.toml
    └── tauri.conf.json
```

---

## License

MIT
