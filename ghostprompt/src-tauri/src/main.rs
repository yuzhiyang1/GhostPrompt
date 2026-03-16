// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod config;
mod llm;
mod stt;

use audio::AudioCapture;
use config::AppConfig;
use llm::LLMClient;
use stt::STTClient;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, GlobalShortcutManager, Manager, Window};

struct AppState {
    audio_capture: Arc<Mutex<AudioCapture>>,
    stt_client: Arc<Mutex<STTClient>>,
    llm_client: Arc<Mutex<LLMClient>>,
    config: Arc<Mutex<AppConfig>>,
    is_click_through: Arc<Mutex<bool>>,
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            setup_app(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            toggle_click_through,
            update_config,
            get_config,
            start_capture,
            stop_capture
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let window = app.get_window("main").unwrap();
    
    // 初始状态：不启用鼠标穿透，让用户可以操作控件
    window.set_ignore_cursor_events(false)?;
    
    // 初始化状态
    let config = AppConfig::load();
    let state = AppState {
        audio_capture: Arc::new(Mutex::new(AudioCapture::new())),
        stt_client: Arc::new(Mutex::new(STTClient::new(&config.deepgram_api_key))),
        llm_client: Arc::new(Mutex::new(LLMClient::new(&config.llm_api_key, &config.llm_base_url, &config.llm_model))),
        config: Arc::new(Mutex::new(config)),
        is_click_through: Arc::new(Mutex::new(false)),
    };
    
    app.manage(state);
    
    // 注册全局快捷键 Ctrl+Shift+G 切换鼠标穿透
    let app_handle = app.handle();
    app.global_shortcut_manager()
        .register("Ctrl+Shift+G", move || {
            if let Some(win) = app_handle.get_window("main") {
                let state: tauri::State<AppState> = app_handle.state();
                let mut is_click_through = state.is_click_through.lock().unwrap();
                *is_click_through = !*is_click_through;
                let val = *is_click_through;
                drop(is_click_through);
                let _ = win.set_ignore_cursor_events(val);
                let _ = win.emit("click-through-changed", val);
            }
        })?;
    
    Ok(())
}

#[tauri::command]
fn toggle_click_through(window: Window, state: tauri::State<AppState>) -> Result<bool, String> {
    toggle_click_through_internal(&window, &state);
    let is_click_through = *state.is_click_through.lock().unwrap();
    Ok(is_click_through)
}

fn toggle_click_through_internal(window: &Window, state: &tauri::State<AppState>) {
    let mut is_click_through = state.is_click_through.lock().unwrap();
    *is_click_through = !*is_click_through;
    
    if let Err(e) = window.set_ignore_cursor_events(*is_click_through) {
        eprintln!("Failed to set ignore cursor events: {}", e);
    }
    
    // 发送事件到前端
    let _ = window.emit("click-through-changed", *is_click_through);
}

#[tauri::command]
fn update_config(
    state: tauri::State<AppState>,
    config_json: String,
) -> Result<(), String> {
    let new_config: AppConfig = serde_json::from_str(&config_json).map_err(|e| e.to_string())?;
    
    // 更新 STT 客户端
    {
        let mut stt = state.stt_client.lock().unwrap();
        *stt = STTClient::new(&new_config.deepgram_api_key);
    }
    
    // 更新 LLM 客户端
    {
        let mut llm = state.llm_client.lock().unwrap();
        *llm = LLMClient::new(&new_config.llm_api_key, &new_config.llm_base_url, &new_config.llm_model);
    }
    
    // 保存配置
    {
        let mut config = state.config.lock().unwrap();
        *config = new_config;
        config.save().map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

#[tauri::command]
fn get_config(state: tauri::State<AppState>) -> Result<String, String> {
    let config = state.config.lock().unwrap();
    serde_json::to_string(&*config).map_err(|e| e.to_string())
}

#[tauri::command]
fn start_capture(app_handle: AppHandle, state: tauri::State<AppState>) -> Result<(), String> {
    let audio_capture = Arc::clone(&state.audio_capture);
    let stt_client = Arc::clone(&state.stt_client);
    let llm_client = Arc::clone(&state.llm_client);
    let (mode, system_prompt, audio_source, debug_mode) = {
        let config = state.config.lock().unwrap();
        (
            config.mode.clone(),
            config.effective_system_prompt(),
            config.audio_source.clone(),
            config.debug_mode,
        )
    };
    
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Err(e) = audio::start_capture_loop(
                app_handle,
                audio_capture,
                stt_client,
                llm_client,
                mode,
                system_prompt,
                audio_source,
                debug_mode,
            ).await {
                eprintln!("Audio capture error: {}", e);
            }
        });
    });
    
    Ok(())
}

#[tauri::command]
fn stop_capture(state: tauri::State<AppState>) -> Result<(), String> {
    let mut audio_capture = state.audio_capture.lock().unwrap();
    audio_capture.stop();
    Ok(())
}
