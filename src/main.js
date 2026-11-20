// 設定視窗：讀取 / 儲存 DeepL 金鑰與快捷鍵
const { invoke } = window.__TAURI__.core;

const form = document.querySelector("#settings-form");
const apiKeyInput = document.querySelector("#api-key");
const toggleKeyButton = document.querySelector("#toggle-key");
const shortcutInput = document.querySelector("#shortcut");
const shortcutStatus = document.querySelector("#shortcut-status");
const message = document.querySelector("#message");

// 修飾鍵本身不能當主按鍵
const MODIFIER_CODES = [
  "ControlLeft", "ControlRight", "AltLeft", "AltRight",
  "ShiftLeft", "ShiftRight", "MetaLeft", "MetaRight",
];

// 把 KeyboardEvent 轉成後端看得懂的快捷鍵字串，例如 "Ctrl+Alt+D"
function toShortcutString(event) {
  if (MODIFIER_CODES.includes(event.code)) return null;

  const parts = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  if (event.metaKey) parts.push("Super");
  if (parts.length === 0) return null;

  // KeyD → D、Digit1 → 1，其餘（F1、Space 等）直接使用 event.code
  const key = event.code.replace(/^Key/, "").replace(/^Digit/, "");
  parts.push(key);
  return parts.join("+");
}

function showMessage(text, isError = false) {
  message.textContent = text;
  message.classList.toggle("error", isError);
}

async function refreshShortcutStatus() {
  const registered = await invoke("get_shortcut_status");
  shortcutStatus.textContent = registered
    ? "✓ 快捷鍵已啟用"
    : "✗ 快捷鍵未啟用，可能已被其他程式佔用，請換一組";
  shortcutStatus.classList.toggle("error", !registered);
}

async function loadSettings() {
  const settings = await invoke("get_settings");
  apiKeyInput.value = settings.apiKey;
  shortcutInput.value = settings.shortcut;
  await refreshShortcutStatus();
}

shortcutInput.addEventListener("keydown", (event) => {
  // Tab 保留給切換焦點
  if (event.key === "Tab") return;
  event.preventDefault();
  const shortcut = toShortcutString(event);
  if (shortcut) shortcutInput.value = shortcut;
});

toggleKeyButton.addEventListener("click", () => {
  const hidden = apiKeyInput.type === "password";
  apiKeyInput.type = hidden ? "text" : "password";
  toggleKeyButton.textContent = hidden ? "隱藏" : "顯示";
});

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  showMessage("");
  try {
    await invoke("save_settings", {
      settings: { apiKey: apiKeyInput.value, shortcut: shortcutInput.value },
    });
    showMessage("已儲存");
  } catch (error) {
    // 後端回傳 Err(String) 時，invoke 會丟出該字串
    showMessage(String(error), true);
  }
  await refreshShortcutStatus();
});

loadSettings().catch((error) => showMessage(`讀取設定失敗：${error}`, true));
