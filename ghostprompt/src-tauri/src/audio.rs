use crate::config::{AppMode, AudioSource};
use crate::llm::LLMClient;
use crate::stt::STTClient;
use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use ringbuf::HeapRb;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use webrtc_vad::{Vad, VadMode};

const SAMPLE_RATE: u32 = 16000;
const FRAME_DURATION_MS: usize = 30;
const SILENCE_THRESHOLD_MS: u64 = 1500;
const MIN_SPEECH_DURATION_MS: u64 = 500;

pub struct AudioCapture {
    is_running: AtomicBool,
}

unsafe impl Send for AudioCapture {}

impl AudioCapture {
    pub fn new() -> Self {
        Self {
            is_running: AtomicBool::new(false),
        }
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
}

pub async fn start_capture_loop(
    app_handle: AppHandle,
    audio_capture: Arc<Mutex<AudioCapture>>,
    stt_client: Arc<Mutex<STTClient>>,
    llm_client: Arc<Mutex<LLMClient>>,
    mode: AppMode,
    system_prompt: String,
    audio_source: AudioSource,
    debug_mode: bool,
) -> Result<()> {
    // 检查 API 配置
    let (deepgram_key, llm_key, llm_base_url, llm_model) = {
        let stt = stt_client.lock().unwrap();
        let llm = llm_client.lock().unwrap();
        (
            stt.api_key.clone(),
            llm.api_key.clone(),
            llm.base_url.clone(),
            llm.model.clone(),
        )
    };
    
    // 验证配置
    let mut missing_configs = Vec::new();
    if deepgram_key.is_empty() {
        missing_configs.push("Deepgram API Key");
    }
    if llm_key.is_empty() {
        missing_configs.push("LLM API Key");
    }
    
    if !missing_configs.is_empty() {
        let error_msg = format!("缺少必要配置: {}", missing_configs.join(", "));
        app_handle.emit_all("debug_log", format!("[ERROR] {}", error_msg)).ok();
        app_handle.emit_all("capture_error", error_msg.clone()).ok();
        return Err(anyhow::anyhow!(error_msg));
    }
    
    if debug_mode {
        app_handle.emit_all("debug_log", "[DEBUG] API 配置检查通过").ok();
        app_handle.emit_all("debug_log", format!("[DEBUG] LLM Base URL: {}", llm_base_url)).ok();
        app_handle.emit_all("debug_log", format!("[DEBUG] LLM Model: {}", llm_model)).ok();
    }
    
    let host = cpal::default_host();
    
    // 根据音频源选择设备
    let device = match audio_source {
        AudioSource::Microphone => {
            let dev = host.default_input_device()
                .ok_or_else(|| anyhow::anyhow!("No input device found"))?;
            if debug_mode {
                let name = dev.name().unwrap_or_default();
                app_handle.emit_all("debug_log", format!("[DEBUG] 使用麦克风: {}", name)).ok();
            }
            dev
        }
        AudioSource::System => {
            // 系统音频捕获需要特殊的设备
            // 在 Windows 上，尝试查找立体声混音设备
            let mut found_device = None;
            for dev in host.input_devices()? {
                if let Ok(name) = dev.name() {
                    let name_lower = name.to_lowercase();
                    if name_lower.contains("stereo mix") || name_lower.contains("立体声混音") 
                        || name_lower.contains("cable") || name_lower.contains("virtual") {
                        found_device = Some(dev);
                        if debug_mode {
                            app_handle.emit_all("debug_log", format!("[DEBUG] 使用系统音频设备: {}", name)).ok();
                        }
                        break;
                    }
                }
            }
            found_device.ok_or_else(|| anyhow::anyhow!(
                "未找到系统音频捕获设备。请确保已启用'立体声混音'或安装虚拟音频设备如 VB-Cable。"
            ))?
        }
    };

    // 配置音频格式 - 使用设备支持的格式
    let default_config = device.default_input_config()?;
    let sample_format = default_config.sample_format();
    let config = StreamConfig {
        channels: default_config.channels(),
        sample_rate: default_config.sample_rate(),
        buffer_size: cpal::BufferSize::Default,
    };

    // 创建音频缓冲区
    let buffer_size = SAMPLE_RATE as usize * 2; // 2秒缓冲区
    let ring_buffer = HeapRb::<f32>::new(buffer_size);
    let (mut producer, mut consumer) = ring_buffer.split();

    // 初始化 VAD
    let mut vad = Vad::new();
    vad.set_mode(VadMode::Quality);

    let is_running = Arc::new(AtomicBool::new(true));

    // 设置音频捕获流
    let stream = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                for &sample in data {
                    let _ = producer.push(sample);
                }
            },
            move |err| {
                eprintln!("Audio stream error: {}", err);
            },
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                for &sample in data {
                    let _ = producer.push(sample as f32 / i16::MAX as f32);
                }
            },
            move |err| {
                eprintln!("Audio stream error: {}", err);
            },
            None,
        ),
        SampleFormat::U16 => device.build_input_stream(
            &config,
            move |data: &[u16], _: &cpal::InputCallbackInfo| {
                for &sample in data {
                    let _ = producer.push((sample as f32 - 32768.0) / 32768.0);
                }
            },
            move |err| {
                eprintln!("Audio stream error: {}", err);
            },
            None,
        ),
        _ => {
            return Err(anyhow::anyhow!("Unsupported sample format: {:?}", sample_format));
        }
    }?;

    stream.play()?;

    {
        let capture = audio_capture.lock().unwrap();
        capture.is_running.store(true, Ordering::SeqCst);
    }

    let source_name = match audio_source {
        AudioSource::Microphone => "麦克风",
        AudioSource::System => "系统音频",
    };
    if debug_mode {
        app_handle.emit_all("debug_log", format!("[DEBUG] 音频捕获已启动: {}", source_name)).ok();
    }

    // 音频处理循环
    let frame_samples = SAMPLE_RATE as usize * FRAME_DURATION_MS / 1000;
    let mut speech_buffer: Vec<f32> = Vec::new();
    let mut is_speaking = false;
    let mut last_speech_time = Instant::now();
    let mut speech_start_time = Instant::now();

    while is_running.load(Ordering::SeqCst) {
        // 检查是否应该停止
        {
            let capture = audio_capture.lock().unwrap();
            if !capture.is_running() {
                break;
            }
        }

        // 收集一帧音频数据
        if consumer.len() >= frame_samples {
            let mut frame: Vec<f32> = Vec::with_capacity(frame_samples);
            for _ in 0..frame_samples {
                if let Some(sample) = consumer.pop() {
                    frame.push(sample);
                }
            }

            // 转换为 i16 用于 VAD
            let frame_i16: Vec<i16> = frame
                .iter()
                .map(|&s| (s * i16::MAX as f32) as i16)
                .collect();

            // VAD 检测
            let is_voice = vad.is_voice_segment(&frame_i16).unwrap_or(false);

            if is_voice {
                if !is_speaking {
                    is_speaking = true;
                    speech_start_time = Instant::now();
                }
                last_speech_time = Instant::now();
                speech_buffer.extend_from_slice(&frame);
            } else if is_speaking {
                speech_buffer.extend_from_slice(&frame);

                // 检查静音时长
                let silence_duration = last_speech_time.elapsed().as_millis() as u64;
                let speech_duration = speech_start_time.elapsed().as_millis() as u64;

                if silence_duration >= SILENCE_THRESHOLD_MS && speech_duration >= MIN_SPEECH_DURATION_MS {
                    // 检测到一段完整的语音，进行处理
                    let audio_data = std::mem::take(&mut speech_buffer);
                    is_speaking = false;

                    println!("Detected speech segment: {}ms", speech_duration);

                    // 发送音频进行 STT 和 LLM 处理
                    let app_handle_clone = app_handle.clone();
                    let stt_client_clone = Arc::clone(&stt_client);
                    let llm_client_clone = Arc::clone(&llm_client);
                    let mode_clone = mode.clone();
                    let system_prompt_clone = system_prompt.clone();

                    tokio::spawn(async move {
                        if let Err(e) = process_audio_segment(
                            app_handle_clone,
                            audio_data,
                            stt_client_clone,
                            llm_client_clone,
                            mode_clone,
                            system_prompt_clone,
                        )
                        .await
                        {
                            eprintln!("Error processing audio: {}", e);
                        }
                    });
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    println!("Audio capture stopped");
    Ok(())
}

async fn process_audio_segment(
    app_handle: AppHandle,
    audio_data: Vec<f32>,
    stt_client: Arc<Mutex<STTClient>>,
    llm_client: Arc<Mutex<LLMClient>>,
    _mode: AppMode,
    system_prompt: String,
) -> Result<()> {
    // 将 f32 音频转换为 i16 PCM
    let pcm_data: Vec<i16> = audio_data
        .iter()
        .map(|&s| (s * i16::MAX as f32) as i16)
        .collect();

    // 转换为字节
    let audio_bytes: Vec<u8> = pcm_data
        .iter()
        .flat_map(|&s| s.to_le_bytes().to_vec())
        .collect();

    // STT 识别 - 提前提取 api_key，释放锁后再 await
    let transcript = {
        let api_key = stt_client.lock().unwrap().api_key.clone();
        let temp_stt = crate::stt::STTClient::new(&api_key);
        match temp_stt.transcribe(&audio_bytes).await {
            Ok(text) => {
                if text.trim().is_empty() {
                    return Ok(());
                }
                println!("STT: {}", text);
                let _ = app_handle.emit_all("stt_result", &text);
                text
            }
            Err(e) => {
                eprintln!("STT error: {}", e);
                return Ok(());
            }
        }
    };

    // LLM 处理 - 提前提取数据，释放锁后再 await
    let (llm_api_key, llm_base_url, llm_model) = {
        let llm = llm_client.lock().unwrap();
        (llm.api_key.clone(), llm.base_url.clone(), llm.model.clone())
    };
    let temp_llm = crate::llm::LLMClient::new(&llm_api_key, &llm_base_url, &llm_model);

    let _ = app_handle.emit_all("llm_start", "");

    match temp_llm.stream_completion(&system_prompt, &transcript).await {
        Ok(mut stream) => {
            use futures::StreamExt;
            
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(text) => {
                        let _ = app_handle.emit_all("llm_token", text);
                    }
                    Err(e) => {
                        eprintln!("LLM stream error: {}", e);
                    }
                }
            }
            
            let _ = app_handle.emit_all("llm_end", "");
        }
        Err(e) => {
            eprintln!("LLM error: {}", e);
            let _ = app_handle.emit_all("llm_error", e.to_string());
        }
    }

    Ok(())
}