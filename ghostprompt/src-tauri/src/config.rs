use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub deepgram_api_key: String,
    /// LLM API Key（适用于任意 OpenAI 兼容接口）
    pub llm_api_key: String,
    /// API Base URL，例如 https://api.openai.com/v1
    /// 也可以填写 DeepSeek / Moonshot / 本地 Ollama 等地址
    #[serde(default = "default_llm_base_url")]
    pub llm_base_url: String,
    /// 模型名称，例如 gpt-4o-mini / deepseek-chat / qwen-plus
    #[serde(default = "default_llm_model")]
    pub llm_model: String,
    pub mode: AppMode,
    pub font_size: u32,
    pub subtitle_position: SubtitlePosition,
    /// 用户自定义 System Prompt（留空时使用内置默认）
    #[serde(default)]
    pub system_prompt: String,
}

fn default_llm_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_llm_model() -> String {
    "gpt-4o-mini".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AppMode {
    #[serde(rename = "translate")]
    Translate,
    #[serde(rename = "interview")]
    Interview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitlePosition {
    pub x: i32,
    pub y: i32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            deepgram_api_key: String::new(),
            llm_api_key: String::new(),
            llm_base_url: default_llm_base_url(),
            llm_model: default_llm_model(),
            mode: AppMode::Translate,
            font_size: 22,
            subtitle_position: SubtitlePosition { x: 100, y: 100 },
            system_prompt: String::new(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let config_path = Self::config_path();
        if config_path.exists() {
            match fs::read_to_string(&config_path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(config) => return config,
                    Err(e) => eprintln!("Failed to parse config: {}", e),
                },
                Err(e) => eprintln!("Failed to read config: {}", e),
            }
        }
        Self::default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let config_path = Self::config_path();
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(config_path, content)?;
        Ok(())
    }

    /// 获取最终生效的 system prompt
    /// 优先使用用户自定义，否则根据模式返回内置默认
    pub fn effective_system_prompt(&self) -> String {
        if !self.system_prompt.trim().is_empty() {
            return self.system_prompt.clone();
        }
        match self.mode {
            AppMode::Translate => {
                "你是一个专业的实时翻译助手。请将以下语音识别文本翻译成中文，语言简洁自然，像正常说话一样口语化，不要直译腔。直接输出译文，不要解释。".to_string()
            }
            AppMode::Interview => {
                "你是一个资深技术面试辅助专家。根据面试官问题的语音转录，给出回答的核心要点。输出要求：口语化、简洁、用 Bullet Points，不超过 100 字，突出专业词汇和解题思路，不要废话。".to_string()
            }
        }
    }

    fn config_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("ghostprompt");
        path.push("config.json");
        path
    }
}
