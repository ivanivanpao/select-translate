// 呼叫 DeepL API 翻譯文字，目標語言固定為繁體中文
// 文件：https://developers.deepl.com/docs/api-reference/translate

use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

const TARGET_LANG: &str = "ZH-HANT";
const FREE_API_URL: &str = "https://api-free.deepl.com/v2/translate";
const PRO_API_URL: &str = "https://api.deepl.com/v2/translate";

// 對應 DeepL 回應的 JSON 結構，只宣告需要的欄位，其餘欄位 serde 會自動忽略：
// { "translations": [ { "detected_source_language": "EN", "text": "..." } ] }
#[derive(Deserialize)]
struct TranslateResponse {
    translations: Vec<Translation>,
}

#[derive(Deserialize)]
struct Translation {
    detected_source_language: String,
    text: String,
}

/// 翻譯結果：偵測到的原文語言代碼（例如 "EN"）與譯文
pub struct TranslateResult {
    pub source_lang: String,
    pub text: String,
}

/// async fn：非同步函式，等待網路回應時不會卡住執行緒，呼叫端要用 .await 取得結果
pub async fn translate(api_key: &str, text: &str) -> Result<TranslateResult, String> {
    // 免費版金鑰結尾是 ":fx"，要打免費版的網址
    let url = if api_key.ends_with(":fx") {
        FREE_API_URL
    } else {
        PRO_API_URL
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .post(url)
        .header("Authorization", format!("DeepL-Auth-Key {api_key}"))
        .json(&json!({ "text": [text], "target_lang": TARGET_LANG }))
        .send()
        .await
        .map_err(|e| format!("無法連線到 DeepL：{e}"))?;

    let status = response.status();
    if !status.is_success() {
        // match 搭配數字：依 HTTP 狀態碼給出看得懂的錯誤訊息
        let message = match status.as_u16() {
            403 => "DeepL API 金鑰無效，請從系統匣開啟「設定」檢查".to_string(),
            429 => "請求太頻繁，請稍後再試".to_string(),
            456 => "本月 DeepL 翻譯額度已用完".to_string(),
            code => format!("DeepL 回傳錯誤（HTTP {code}）"),
        };
        return Err(message);
    }

    let data: TranslateResponse = response
        .json()
        .await
        .map_err(|e| format!("無法解析 DeepL 回應：{e}"))?;

    // into_iter().next()：取走 Vec 的第一個元素；Vec 是空的就得到 None
    // ok_or：把 None 轉成 Err(訊息)，再用 ? 往外回傳
    let first = data
        .translations
        .into_iter()
        .next()
        .ok_or("DeepL 沒有回傳翻譯結果")?;

    Ok(TranslateResult {
        source_lang: first.detected_source_language,
        text: first.text,
    })
}
