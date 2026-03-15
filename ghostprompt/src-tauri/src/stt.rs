use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;

pub struct STTClient {
    pub api_key: String,
    client: Client,
}

#[derive(Debug, Deserialize)]
struct DeepgramResponse {
    results: Option<Results>,
}

#[derive(Debug, Deserialize)]
struct Results {
    channels: Vec<Channel>,
}

#[derive(Debug, Deserialize)]
struct Channel {
    alternatives: Vec<Alternative>,
}

#[derive(Debug, Deserialize)]
struct Alternative {
    transcript: String,
}

impl STTClient {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            client: Client::new(),
        }
    }

    pub async fn transcribe(&self, audio_data: &[u8]) -> Result<String> {
        if self.api_key.is_empty() {
            return Err(anyhow::anyhow!("Deepgram API key not configured"));
        }

        // 将 PCM 数据转换为 WAV 格式
        let wav_data = self.pcm_to_wav(audio_data)?;

        let url = "https://api.deepgram.com/v1/listen?model=nova-2&language=zh&punctuate=true";

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Token {}", self.api_key))
            .header("Content-Type", "audio/wav")
            .body(wav_data)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Deepgram API error: {}", error_text));
        }

        let result: DeepgramResponse = response.json().await?;

        let transcript = result
            .results
            .and_then(|r| r.channels.into_iter().next())
            .and_then(|c| c.alternatives.into_iter().next())
            .map(|a| a.transcript)
            .unwrap_or_default();

        Ok(transcript)
    }

    fn pcm_to_wav(&self, pcm_data: &[u8]) -> Result<Vec<u8>> {
        let sample_rate = 16000u32;
        let channels = 1u16;
        let bits_per_sample = 16u16;
        let byte_rate = sample_rate * channels as u32 * (bits_per_sample as u32 / 8);
        let block_align = channels * (bits_per_sample / 8);

        let data_len = pcm_data.len() as u32;
        let file_len = 36 + data_len;

        let mut wav = Vec::with_capacity((44 + data_len) as usize);

        // RIFF header
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&file_len.to_le_bytes());
        wav.extend_from_slice(b"WAVE");

        // fmt chunk
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size
        wav.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat (PCM)
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&bits_per_sample.to_le_bytes());

        // data chunk
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        wav.extend_from_slice(pcm_data);

        Ok(wav)
    }
}
