# CLAUDE.md

本檔提供 Claude Code 在此專案工作時的指引。專案目標、架構圖與進度清單詳見 `README.md`。

## 專案概要

Windows 劃詞翻譯工具：在任何應用程式選取文字 → 按全域快捷鍵 → 模擬 Ctrl+C 取得選取文字 → 呼叫 DeepL API → 在滑鼠游標旁彈出永遠置頂的浮動視窗顯示譯文。App 以系統匣常駐方式執行。

## 技術棧

- Tauri 2.x（後端 Rust，前端使用系統 WebView2）
- 前端：純 HTML / CSS / JavaScript，**不使用任何前端框架、不使用 TypeScript、不使用打包工具**
- 套件管理：npm（前端 / CLI）、Cargo（Rust）
- 目標平台：僅 Windows

## 常用指令

```bash
npm run tauri dev      # 開發模式啟動（會編譯 Rust 並開啟 App）
npm run tauri build    # 建置正式安裝檔
```

Rust 端檢查（在 `src-tauri/` 目錄下執行）：

```bash
cargo check            # 型別 / 編譯檢查，修改 Rust 程式後優先用這個驗證
```

目前專案沒有測試與 lint 設定。

## 專案結構

- `src/`：前端靜態檔案，`tauri.conf.json` 的 `frontendDist` 直接指向此目錄，沒有開發伺服器
- `src-tauri/src/lib.rs`：Tauri Builder、plugin 註冊、`#[tauri::command]` 指令都放這裡
- `src-tauri/src/main.rs`：只負責呼叫 `select_translate_lib::run()`，不放邏輯
- `src-tauri/tauri.conf.json`：視窗、打包設定
- `src-tauri/capabilities/`：前端可使用的權限；新增 plugin 後，前端要用到的權限必須加進這裡

## 開發慣例

- 前端透過 `withGlobalTauri: true` 提供的全域物件呼叫 API（例如 `window.__TAURI__.core.invoke`），不要引入 `@tauri-apps/api` 的 ES module 寫法
- 預定使用的套件：`tauri-plugin-global-shortcut`（全域快捷鍵）、`tauri-plugin-clipboard-manager`（剪貼簿）、`enigo`（模擬按鍵）；新增其他相依性前先與使用者確認
- 使用者正在學習 Rust（先前只讀過、沒寫過），實作 Rust 程式時請搭配說明語法與所有權等概念，避免過度抽象或過於進階的寫法

## 已確定的設計決策（修改前需與使用者確認）

- 系統匣常駐，不顯示一般主視窗、不佔工作列
- 模擬 Ctrl+C 前先保存原剪貼簿內容，翻譯完成後寫回（剪貼簿還原邏輯從第一版就要有）
- 模擬複製後需短暫延遲（約 50–150ms）再讀取剪貼簿，避免讀到舊內容
- 翻譯 API 使用 DeepL，目標語言 `ZH-HANT`
- DeepL API 金鑰不得寫死在程式碼或提交進版控
