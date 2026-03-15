use anyhow::Result;
use futures::Stream;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

pub struct LLMClient {
    pub api_key: String,
    /// OpenAI 兼容的 Base URL，例如：
    ///   https://api.openai.com/v1
    ///   https://api.deepseek.com/v1
    ///   https://api.moonshot.cn/v1
    ///   http://localhost:11434/v1  (Ollama)
    pub base_url: String,
    /// 模型名称，例如 gpt-4o-mini / deepseek-chat / moonshot-v1-8k
    pub model: String,
    client: Client,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    temperature: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct StreamResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    delta: Delta,
}

#[derive(Debug, Deserialize, Default)]
struct Delta {
    content: Option<String>,
}

pub type LLMStream = Pin<Box<dyn Stream<Item = Result<String>> + Send>>;

impl LLMClient {
    pub fn new(api_key: &str, base_url: &str, model: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            client: Client::new(),
        }
    }

    pub async fn stream_completion(
        &self,
        system_prompt: &str,
        user_message: &str,
    ) -> Result<LLMStream> {
        if self.api_key.is_empty() {
            return Err(anyhow::anyhow!("LLM API key not configured"));
        }

        let endpoint = format!("{}/chat/completions", self.base_url);

        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: user_message.to_string(),
                },
            ],
            stream: true,
            temperature: 0.7,
        };

        let response = self
            .client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("LLM API error ({}): {}", status, error_text));
        }

        let stream = response.bytes_stream();

        let output_stream = async_stream::try_stream! {
            use futures::StreamExt;

            let mut stream = stream;
            let mut buffer = String::new();

            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                let text = String::from_utf8_lossy(&chunk);
                buffer.push_str(&text);

                // 处理 SSE 格式：以 \n\n 为事件边界
                while let Some(pos) = buffer.find("\n\n") {
                    let block = buffer[..pos].to_string();
                    buffer = buffer[pos + 2..].to_string();

                    for line in block.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data.trim() == "[DONE]" {
                                continue;
                            }
                            if let Ok(resp) = serde_json::from_str::<StreamResponse>(data) {
                                if let Some(choice) = resp.choices.first() {
                                    if let Some(content) = &choice.delta.content {
                                        if !content.is_empty() {
                                            yield content.clone();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // 处理缓冲区中的剩余数据
            for line in buffer.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if data.trim() == "[DONE]" { continue; }
                    if let Ok(resp) = serde_json::from_str::<StreamResponse>(data) {
                        if let Some(choice) = resp.choices.first() {
                            if let Some(content) = &choice.delta.content {
                                if !content.is_empty() {
                                    yield content.clone();
                                }
                            }
                        }
                    }
                }
            }
        };

        Ok(Box::pin(output_stream))
    }
}
