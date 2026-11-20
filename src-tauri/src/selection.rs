// 取得使用者在任意應用程式中選取的文字：
// 保存原剪貼簿 → 模擬 Ctrl+C → 等剪貼簿更新後讀取 → 寫回原剪貼簿

use enigo::{Direction, Enigo, Key, Keyboard, Settings as EnigoSettings};
use std::thread;
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{Modifiers, Shortcut};

// 模擬複製後，每隔多久檢查一次剪貼簿、最多檢查幾次（50ms × 10 = 最多等 0.5 秒）
const POLL_INTERVAL: Duration = Duration::from_millis(50);
const POLL_TIMES: u32 = 10;

/// 回傳 Ok(Some(文字)) 表示取得選取文字；Ok(None) 表示沒有選取任何文字。
/// 這個函式會用 thread::sleep 等待，屬於「阻塞」操作，呼叫端要放在背景執行緒執行。
pub fn copy_selected_text(app: &AppHandle, shortcut: &Shortcut) -> Result<Option<String>, String> {
    let clipboard = app.clipboard();

    // 1. 記住原本的剪貼簿文字。剪貼簿是空的或不是文字（例如圖片）時，read_text 會失敗 → None
    //    .ok() 把 Result<String, _> 轉成 Option<String>，丟掉錯誤細節
    let original: Option<String> = clipboard.read_text().ok();

    // 2. 原本有文字時先清空，之後才能分辨「Ctrl+C 真的複製到東西」還是「讀到舊內容」。
    //    原本不是文字時不清空，這樣沒選到文字的情況下，使用者的圖片等內容不會被動到。
    if original.is_some() {
        clipboard.clear().map_err(|e| e.to_string())?;
    }

    // 3. 模擬 Ctrl+C，並等剪貼簿更新（複製後不會立刻更新，要稍等一下才讀得到）
    simulate_copy(shortcut)?;

    let mut copied: Option<String> = None;
    for _ in 0..POLL_TIMES {
        thread::sleep(POLL_INTERVAL);
        if let Ok(text) = clipboard.read_text() {
            if !text.trim().is_empty() {
                copied = Some(text);
                break;
            }
        }
    }

    // 4. 讀到選取文字後立刻寫回原本的剪貼簿，不必等翻譯完成，縮短使用者剪貼簿被佔用的時間。
    //    &original：借用 original 裡的字串來讀取，不取走它的所有權
    if let Some(text) = &original {
        if let Err(e) = clipboard.write_text(text.as_str()) {
            eprintln!("還原剪貼簿失敗：{e}");
        }
    }

    Ok(copied)
}

/// 送出 Ctrl+C。
/// 觸發時使用者可能還按著快捷鍵的修飾鍵（例如 Ctrl+Alt+D 的 Alt），
/// 直接送 C 會變成 Ctrl+Alt+C，所以要先把這些鍵放開。
fn simulate_copy(shortcut: &Shortcut) -> Result<(), String> {
    // let mut：Rust 的變數預設不可變，要呼叫會修改自身的方法（enigo.key）就得宣告成 mut
    let mut enigo = Enigo::new(&EnigoSettings::default()).map_err(|e| e.to_string())?;

    // 先按住 Ctrl，再放開其他修飾鍵：
    // 在 Ctrl 按著的狀態下放開 Alt，不會觸發應用程式的選單列（單獨放開 Alt 會）
    press(&mut enigo, Key::Control, Direction::Press)?;
    if shortcut.mods.contains(Modifiers::ALT) {
        press(&mut enigo, Key::Alt, Direction::Release)?;
    }
    if shortcut.mods.contains(Modifiers::SHIFT) {
        // 不放開 Shift 的話，瀏覽器會收到 Ctrl+Shift+C（開啟開發者工具）
        press(&mut enigo, Key::Shift, Direction::Release)?;
    }
    if shortcut.mods.contains(Modifiers::SUPER) {
        press(&mut enigo, Key::Meta, Direction::Release)?;
    }
    press(&mut enigo, Key::C, Direction::Click)?;
    press(&mut enigo, Key::Control, Direction::Release)?;
    Ok(())
}

// 小工具函式：把 enigo 的錯誤型別轉成 String，讓上面每一行都能直接用 ?
// &mut Enigo：可變借用，函式可以操作 enigo，但所有權仍屬於呼叫端
fn press(enigo: &mut Enigo, key: Key, direction: Direction) -> Result<(), String> {
    enigo.key(key, direction).map_err(|e| e.to_string())
}
