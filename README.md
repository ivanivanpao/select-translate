# Select-Translate

Windows 劃詞翻譯工具。在任何應用程式中選取文字，按下快捷鍵，就會在滑鼠游標旁彈出繁體中文譯文。

- 不限應用程式：瀏覽器、PDF 閱讀器、Word 等，只要能複製文字就能翻譯
- 使用 DeepL 翻譯，目標語言為繁體中文
- 常駐在系統匣，平常不佔畫面，也不佔工作列
- 翻譯完成後會還原你原本的剪貼簿內容

## 使用方式

1. 啟動 App 後，系統匣會出現圖示。第一次使用時會自動開啟設定視窗。
2. 在設定視窗貼上你的 DeepL API 金鑰並儲存。金鑰可在 [DeepL 帳戶](https://www.deepl.com/your-account/keys) 取得，免費方案每月 50 萬字元。
3. 在任何程式中選取文字，按 `Ctrl+Alt+D`（預設快捷鍵），譯文會顯示在游標旁。
4. 按 `Esc`、點視窗外面或按 ✕ 關閉譯文視窗；按「複製」可以複製譯文。

系統匣圖示：

- 左鍵：開啟設定（修改金鑰、快捷鍵）
- 右鍵：選單（設定、結束）

設定存放在 `%APPDATA%\com.ivani.select-translate\settings.json`。

## 開發

### 環境需求

- Windows 10 / 11（需要 WebView2，Windows 11 已內建）
- [Node.js](https://nodejs.org/)
- [Rust](https://www.rust-lang.org/tools/install)（MSVC 工具鏈）
- 建議的 VS Code 擴充套件：[Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)、[rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

### 指令

```bash
npm install            # 安裝前端相依套件（第一次）
npm run tauri dev      # 開發模式啟動
npm run tauri build    # 建置正式安裝檔

cd src-tauri
cargo check            # Rust 編譯 / 型別檢查
```

第一次編譯 Rust 相依套件會花幾分鐘，屬正常現象。

### 專案結構

```
src/                  前端（純 HTML / CSS / JS，無框架、無打包工具）
  index.html          設定視窗
  popup.html          浮動翻譯視窗
src-tauri/
  src/lib.rs          App 進入點：快捷鍵、系統匣、翻譯流程、前端指令
  src/selection.rs    模擬 Ctrl+C 取得選取文字、還原剪貼簿
  src/deepl.rs        DeepL API
  src/settings.rs     設定讀寫
  tauri.conf.json     視窗與打包設定
```

## 已知限制

- 少數防拷貝的內容（受保護的 PDF 等）無法用複製取得文字
- 以系統管理員身分執行的程式，無法從一般權限的 App 模擬按鍵
- 剪貼簿還原只支援文字：如果你原本複製的是圖片，選取文字翻譯後圖片會被覆蓋
- 單次最多翻譯 5000 字

設計決策、架構與實作細節請見 [docs/design.md](docs/design.md)。
