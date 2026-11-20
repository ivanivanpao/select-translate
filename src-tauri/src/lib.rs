// mod：宣告同資料夾下的模組檔案（settings.rs、selection.rs、deepl.rs）
mod deepl;
mod selection;
mod settings;

use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, State, WebviewWindow,
    WindowEvent,
};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use settings::Settings;

// 浮動視窗寬度（邏輯像素，會依螢幕縮放比例放大）與離游標的距離
const POPUP_WIDTH: f64 = 380.0;
const POPUP_OFFSET: f64 = 16.0;
// 一次最多翻譯的字數，避免誤選整頁內容而耗光 DeepL 額度
const MAX_CHARS: usize = 5000;

/// 整個 App 共用的狀態，透過 app.manage() 交給 Tauri 保管，之後用 app.state::<AppState>() 取得。
/// 多個執行緒可能同時存取，所以要修改的資料都包在 Mutex（互斥鎖）裡：同一時間只有一個執行緒能 lock 成功。
struct AppState {
    settings: Mutex<Settings>,
    // 浮動視窗要貼齊的游標位置（實體像素）
    popup_anchor: Mutex<PhysicalPosition<f64>>,
    // 確保同一時間只有一個翻譯在操作剪貼簿，避免連按快捷鍵時還原錯內容
    clipboard_lock: Mutex<()>,
    // 每次觸發翻譯就 +1；翻譯完成時若編號已不是最新，代表使用者又觸發了新的翻譯，舊結果就丟掉
    request_id: AtomicU64,
}

/// 傳給浮動視窗前端的狀態。
/// #[serde(tag = "status")]：轉成 JSON 時多一個 "status" 欄位標示是哪一種，例如
/// { "status": "done", "source": "...", "translation": "...", "lang": "EN" }
#[derive(Clone, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
enum PopupState {
    Loading { source: String },
    Done { source: String, translation: String, lang: String },
    Error { message: String },
}

// ───────────── 翻譯流程 ─────────────

/// 快捷鍵放開時呼叫。真正的工作丟到背景非同步任務，不卡住 Tauri 的主執行緒（事件迴圈）
fn start_translation(app: &AppHandle, shortcut: Shortcut) {
    // AppHandle 可以便宜地 clone，clone 出來的副本「移動」(move) 進背景任務裡使用
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        run_translation(app, shortcut).await;
    });
}

async fn run_translation(app: AppHandle, shortcut: Shortcut) {
    let state = app.state::<AppState>();
    // fetch_add 回傳加之前的值，所以 +1 才是這次的編號
    let my_id = state.request_id.fetch_add(1, Ordering::SeqCst) + 1;
    let anchor = app.cursor_position().unwrap_or_default();

    // 1. 取得選取文字。模擬按鍵與等待剪貼簿會阻塞執行緒，所以用 spawn_blocking 放到專門的執行緒
    let app_for_copy = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let state = app_for_copy.state::<AppState>();
        // _guard 存活期間持有鎖，離開這個區塊時自動釋放（Rust 的 RAII：值被丟棄時自動清理）
        let _guard = state.clipboard_lock.lock().unwrap();
        selection::copy_selected_text(&app_for_copy, &shortcut)
    })
    .await;

    // spawn_blocking 本身也可能失敗，結果是兩層 Result，這裡攤平成一層
    let copied = match result {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    };

    let text = match copied {
        Ok(Some(text)) => text.trim().to_string(),
        Ok(None) => {
            show_popup(&app, anchor, PopupState::Error {
                message: "沒有偵測到選取的文字".to_string(),
            });
            return;
        }
        Err(e) => {
            show_popup(&app, anchor, PopupState::Error {
                message: format!("讀取選取文字失敗：{e}"),
            });
            return;
        }
    };

    // chars().count() 算的是字元數；len() 算的是位元組數，中文一個字佔 3 個位元組
    if text.chars().count() > MAX_CHARS {
        show_popup(&app, anchor, PopupState::Error {
            message: format!("選取的文字超過 {MAX_CHARS} 字，請縮小範圍"),
        });
        return;
    }

    // 2. 檢查金鑰。lock() 取得的鎖在這行結束就釋放，不會一直佔著
    let api_key = state.settings.lock().unwrap().api_key.clone();
    if api_key.is_empty() {
        show_popup(&app, anchor, PopupState::Error {
            message: "尚未設定 DeepL API 金鑰，已開啟設定視窗".to_string(),
        });
        show_settings(&app);
        return;
    }

    // 3. 先顯示「翻譯中」，再呼叫 DeepL
    show_popup(&app, anchor, PopupState::Loading { source: text.clone() });
    let popup_state = match deepl::translate(&api_key, &text).await {
        Ok(result) => PopupState::Done {
            source: text,
            translation: result.text,
            lang: result.source_lang,
        },
        Err(message) => PopupState::Error { message },
    };

    // 4. 使用者在等待期間又觸發了新的翻譯，就不要用舊結果蓋掉新的
    if state.request_id.load(Ordering::SeqCst) == my_id {
        update_popup(&app, popup_state);
    }
}

// ───────────── 浮動視窗 ─────────────

/// 把浮動視窗移到游標旁、更新內容並顯示
fn show_popup(app: &AppHandle, anchor: PhysicalPosition<f64>, state: PopupState) {
    *app.state::<AppState>().popup_anchor.lock().unwrap() = anchor;
    update_popup(app, state);

    if let Some(popup) = app.get_webview_window("popup") {
        if let Ok(size) = popup.outer_size() {
            place_popup(app, &popup, size);
        }
        let _ = popup.show();
        let _ = popup.set_focus();
    }
}

/// 只更新浮動視窗內容（送事件給前端），不移動、不顯示
fn update_popup(app: &AppHandle, state: PopupState) {
    if let Err(e) = app.emit_to("popup", "popup-state", state) {
        eprintln!("傳送浮動視窗內容失敗：{e}");
    }
}

/// 計算浮動視窗位置：預設在游標右下方，超出螢幕就翻到另一側，最後再限制在螢幕工作區（不含工作列）內
fn place_popup(app: &AppHandle, popup: &WebviewWindow, size: PhysicalSize<u32>) {
    let anchor = *app.state::<AppState>().popup_anchor.lock().unwrap();
    let (w, h) = (size.width as i32, size.height as i32);

    // monitor_from_point 回傳 Result<Option<Monitor>>，兩層都成功才拿得到螢幕資訊
    let monitor = match app.monitor_from_point(anchor.x, anchor.y) {
        Ok(Some(monitor)) => monitor,
        _ => {
            let _ = popup.set_position(PhysicalPosition::new(anchor.x as i32, anchor.y as i32));
            return;
        }
    };

    let offset = (POPUP_OFFSET * monitor.scale_factor()) as i32;
    let area = monitor.work_area();
    let left = area.position.x;
    let top = area.position.y;
    let right = left + area.size.width as i32;
    let bottom = top + area.size.height as i32;

    let mut x = anchor.x as i32 + offset;
    let mut y = anchor.y as i32 + offset;
    if x + w > right {
        x = anchor.x as i32 - offset - w;
    }
    if y + h > bottom {
        y = anchor.y as i32 - offset - h;
    }
    // clamp(最小值, 最大值)：把數值限制在範圍內；.max(left) 避免視窗比螢幕還寬時範圍顛倒
    x = x.clamp(left, (right - w).max(left));
    y = y.clamp(top, (bottom - h).max(top));

    let _ = popup.set_position(PhysicalPosition::new(x, y));
}

// ───────────── 設定視窗與系統匣 ─────────────

fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let settings_item = MenuItem::with_id(app, "settings", "設定", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "結束", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings_item, &quit_item])?;

    let mut builder = TrayIconBuilder::new()
        .tooltip("Select-Translate 劃詞翻譯")
        .menu(&menu)
        // 左鍵直接開設定，右鍵才顯示選單
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => show_settings(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // 用模式比對 (pattern matching) 只處理「左鍵放開」，.. 表示忽略其他欄位
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_settings(tray.app_handle());
            }
        });

    // 使用 App 本身的圖示（src-tauri/icons）
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

// ───────────── 前端可呼叫的指令 ─────────────

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

/// 目前設定的快捷鍵是否註冊成功（可能被其他程式佔用）
#[tauri::command]
fn get_shortcut_status(app: AppHandle, state: State<AppState>) -> bool {
    let shortcut = state.settings.lock().unwrap().shortcut.clone();
    app.global_shortcut().is_registered(shortcut.as_str())
}

#[tauri::command]
fn save_settings(app: AppHandle, state: State<AppState>, settings: Settings) -> Result<(), String> {
    let settings = Settings {
        api_key: settings.api_key.trim().to_string(),
        shortcut: settings.shortcut.trim().to_string(),
    };

    // parse()：把字串解析成 Shortcut，格式錯誤就回傳錯誤給前端
    let new_shortcut: Shortcut = settings
        .shortcut
        .parse()
        .map_err(|e| format!("快捷鍵格式不正確：{e}"))?;

    // 換掉快捷鍵：先全部取消註冊再註冊新的；新的註冊失敗時改回舊的
    let old_shortcut = state.settings.lock().unwrap().shortcut.clone();
    let global_shortcut = app.global_shortcut();
    global_shortcut.unregister_all().map_err(|e| e.to_string())?;
    if let Err(e) = global_shortcut.register(new_shortcut) {
        let _ = global_shortcut.register(old_shortcut.as_str());
        return Err(format!("無法註冊快捷鍵 {}（可能已被其他程式使用）：{e}", settings.shortcut));
    }

    settings::save(&app, &settings)?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

#[tauri::command]
fn hide_popup(app: AppHandle) {
    if let Some(popup) = app.get_webview_window("popup") {
        let _ = popup.hide();
    }
}

/// 前端排版完成後回報內容高度（邏輯像素），依此調整浮動視窗大小並重新定位
#[tauri::command]
fn fit_popup(app: AppHandle, height: f64) {
    if let Some(popup) = app.get_webview_window("popup") {
        let scale = popup.scale_factor().unwrap_or(1.0);
        let _ = popup.set_size(LogicalSize::new(POPUP_WIDTH, height));
        // set_size 不一定立刻生效，所以自己算出實體像素大小拿來定位
        let size = PhysicalSize::new((POPUP_WIDTH * scale) as u32, (height * scale) as u32);
        place_popup(&app, &popup, size);
    }
}

#[tauri::command]
fn copy_text(app: AppHandle, text: String) -> Result<(), String> {
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

// ───────────── 進入點 ─────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    // 在「放開」時才觸發，此時主按鍵已放開，模擬 Ctrl+C 比較不會被干擾
                    if event.state() == ShortcutState::Released {
                        start_translation(app, *shortcut);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let loaded = settings::load(app.handle());
            let mut need_settings = loaded.api_key.is_empty();

            if let Err(e) = app.global_shortcut().register(loaded.shortcut.as_str()) {
                eprintln!("註冊快捷鍵 {} 失敗：{e}", loaded.shortcut);
                need_settings = true;
            }

            app.manage(AppState {
                settings: Mutex::new(loaded),
                popup_anchor: Mutex::new(PhysicalPosition::default()),
                clipboard_lock: Mutex::new(()),
                request_id: AtomicU64::new(0),
            });

            build_tray(app)?;

            // 第一次使用（沒有金鑰）或快捷鍵註冊失敗時，主動打開設定視窗
            if need_settings {
                show_settings(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // 關閉設定視窗時只隱藏，App 繼續在系統匣常駐
            WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                api.prevent_close();
                let _ = window.hide();
            }
            // 浮動視窗失去焦點（使用者點了別的地方）就自動隱藏
            WindowEvent::Focused(false) if window.label() == "popup" => {
                let _ = window.hide();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            get_shortcut_status,
            save_settings,
            hide_popup,
            fit_popup,
            copy_text
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
