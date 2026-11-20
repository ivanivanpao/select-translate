// 使用者設定：DeepL API 金鑰與全域快捷鍵
// 存放位置：%APPDATA%\com.ivani.select-translate\settings.json（不在專案資料夾內，不會進版控）

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub const DEFAULT_SHORTCUT: &str = "Ctrl+Alt+D";

// derive 讓編譯器自動產生實作：
// - Clone：可以用 .clone() 複製一份
// - Serialize / Deserialize：可以和 JSON 互相轉換（serde 套件）
#[derive(Debug, Clone, Serialize, Deserialize)]
// 轉成 JSON 時欄位改用 camelCase（api_key → apiKey），符合 JS 的命名習慣
#[serde(rename_all = "camelCase")]
// 設定檔缺少某個欄位時，用 Default 的值補上，而不是整個讀取失敗
#[serde(default)]
pub struct Settings {
    pub api_key: String,
    pub shortcut: String,
}

// 手動實作 Default，讓快捷鍵的預設值是 Ctrl+Alt+D 而不是空字串
impl Default for Settings {
    fn default() -> Self {
        Settings {
            api_key: String::new(),
            shortcut: DEFAULT_SHORTCUT.to_string(),
        }
    }
}

// 取得設定檔路徑。回傳 Result：成功是 Ok(路徑)，失敗是 Err(錯誤訊息)
fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    // ? 運算子：如果是 Err 就直接把錯誤往外回傳，是 Ok 就取出裡面的值繼續執行
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}

/// 讀取設定。讀不到或格式錯誤都退回預設值，避免設定檔壞掉導致 App 無法啟動
pub fn load(app: &AppHandle) -> Settings {
    let path = match settings_path(app) {
        Ok(path) => path,
        Err(_) => return Settings::default(),
    };

    match fs::read_to_string(&path) {
        // unwrap_or_default：解析失敗時改用 Settings::default()
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

/// 把設定寫入檔案；資料夾不存在時會先建立
pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = settings_path(app)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| e.to_string())
}
