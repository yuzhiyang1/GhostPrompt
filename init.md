桌面端 AI 实时字幕与面试辅助工具 - PRD & 技术设计文档
第一部分：产品需求文档 (PRD)
1. 产品概述
本产品是一款轻量级、无状态（Stateless）的桌面端辅助工具。旨在用户进行音视频通话、在线会议或远程面试时，实时捕获系统扬声器及麦克风音频，利用云端大模型进行低延迟的语音转文字（STT）和意图处理（翻译/面试解答），并以“鼠标穿透的透明悬浮字幕”形式展示在桌面最上层。

2. 目标用户与使用场景
个人自用：不涉及多账户系统、无需云端历史记录同步。

场景 A（跨语言沟通）：外语会议时，实时将对方语音翻译为中文。

场景 B（面试辅助）：远程面试时，监听面试官问题，实时在屏幕上生成简短的答题提示（八股文要点、思路引导）。

3. 核心功能需求 (MVP)
F1. 幽灵悬浮窗 (Ghost UI)：

窗口必须无边框、背景完全透明。

永远置顶 (Always on top)。

鼠标穿透 (Click-through)：用户点击字幕区域时，事件必须穿透到下层的会议软件（如腾讯会议、Zoom），不能阻挡用户操作。

F2. 双路音频捕获：

实时截取系统扬声器输出（内录对方声音）。

可选：截取麦克风输入（录制自己声音，用于补充上下文）。

F3. 智能静音检测 (VAD)：

根据音量阈值和静音时长（如 1~1.5 秒）自动对连续的音频流进行断句截取，避免频繁无效的 API 请求。

F4. 极速语音转文本 (STT)：

对接云端 STT API（如 Deepgram），要求低延迟，快速返回文本。

F5. 大模型流式处理 (LLM Streaming)：

根据预设的 Prompt（翻译模式/面试模式），将识别出的文本发送至 LLM（如 Claude 3.5 Sonnet / GPT-4o）。

必须支持流式输出 (Server-Sent Events)，在前端实现打字机效果，降低用户等待焦虑。

F6. 本地配置管理：

提供一个极简的设置界面（或直接通过本地配置文件/LocalStorage），用于配置 API Key、切换模式（更改 System Prompt）、调整字体大小和字幕位置。

4. 非功能需求 (约束)
无需后端服务器：纯客户端应用，所有状态和配置保存在本地。

跨平台兼容：需同时支持 Windows 10/11 和 macOS。

低资源占用：长期后台运行，内存占用需控制在最低限度。

第二部分：技术设计文档 (TDD)
1. 技术栈选型
宿主框架：Tauri (轻量、跨平台、内存占用极低)。

前端技术：HTML + CSS + 原生 JS (或轻量级 Vue/React)。

后端技术 (本地)：Rust。

第三方服务：

STT 服务：Deepgram API (REST or WebSocket)。

LLM 服务：OpenAI / Anthropic API (REST streaming)。

2. 系统架构与数据流
系统分为 Rust Core（处理系统级交互和网络）和 Web UI（纯展示）两部分。

音频采集 (Rust)：使用 cpal 库实时监听系统音频。

VAD 缓冲 (Rust)：在内存中维护一个 PCM 音频 Buffer。计算 RMS 音量，当检测到有效语音并出现设定时长的静音后，封包此段 Buffer。

STT 请求 (Rust)：使用 reqwest 将音频数据发送给 Deepgram API，获取转录文本。

LLM 请求 (Rust)：将转录文本结合当前模式的 Prompt，使用 reqwest 发送给大模型 API，开启 stream: true。

IPC 通信 (Tauri)：Rust 端接收到 LLM 的流式返回块 (chunks) 后，通过 tauri::AppHandle::emit_all 触发自定义事件（如 llm_token）。

前端渲染 (Web)：前端监听 llm_token 事件，将接收到的文本追加到 DOM 中，配合 CSS 文本阴影确保可读性。

3. 核心模块开发说明 (致开发者)
3.1 跨平台音频捕获 (Critical)
Windows：使用 cpal 并指定 HostId::Wasapi。利用 WASAPI 的 Loopback 特性直接捕获扬声器默认输出。

macOS：苹果系统禁止直接内录扬声器。必须在文档中要求用户安装虚拟声卡（如 BlackHole 2ch）。代码端需实现寻找并监听名为 "BlackHole" 的输入设备。

3.2 窗口属性与系统 API交互
透明与置顶：在 tauri.conf.json 中配置 "transparent": true 和 "alwaysOnTop": true。

鼠标穿透实现：在 Rust的 setup 生命周期中，获取 window 实例，并调用 window.set_ignore_cursor_events(true)。

注意：需要提供一个全局快捷键（如利用 rdev 或 tauri 的 global-shortcut 插件）来切换鼠标穿透状态，以便用户偶尔可以拖动调整窗口位置。

3.3 Prompt 策略设计 (参考)
面试模式 System Prompt：“你是一个资深的软件技术面试辅助专家。请根据提供的面试官语音转录文本，提炼出回答此问题的核心要点。以极简的 Bullet Points 形式输出，总字数不超过 100 字，重点突出专业词汇和解题思路，不要说废话。”

4. 交付里程碑建议
阶段 1 (UI & 框架)：搭建 Tauri 项目，实现前端透明悬浮窗及鼠标穿透。配置本地 LocalStorage 存储 API Key。

阶段 2 (音频链路)：Rust 端实现跨平台音频录制及基础 VAD（写出本地音频文件测试是否录制成功）。

阶段 3 (API 对接)：集成 Deepgram 与 LLM 流式请求，通过 IPC 打通 Rust 到 Web 的数据流。

阶段 4 (联调与优化)：调优 VAD 断句的延迟时长，优化前端打字机渲染的平滑度。