// 浮動翻譯視窗：接收後端送來的狀態（翻譯中 / 完成 / 錯誤）並顯示
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const card = document.querySelector("#card");
const langLabel = document.querySelector("#lang");
const translationText = document.querySelector("#translation");
const sourceText = document.querySelector("#source");
const copyButton = document.querySelector("#copy");
const closeButton = document.querySelector("#close");

// 浮動視窗最高高度（邏輯像素），內容更長時在視窗內捲動
const MAX_HEIGHT = 420;

// DeepL 回傳的原文語言代碼 → 中文名稱，沒列到的直接顯示代碼
const LANG_NAMES = {
  EN: "英文", JA: "日文", KO: "韓文", ZH: "中文", DE: "德文", FR: "法文",
  ES: "西班牙文", IT: "義大利文", PT: "葡萄牙文", RU: "俄文", NL: "荷蘭文",
  PL: "波蘭文", TR: "土耳其文", UK: "烏克蘭文", ID: "印尼文", VI: "越南文",
  TH: "泰文", AR: "阿拉伯文",
};

function render(state) {
  card.dataset.status = state.status;
  copyButton.hidden = state.status !== "done";
  copyButton.textContent = "複製";

  if (state.status === "loading") {
    langLabel.textContent = "翻譯中…";
    translationText.textContent = "";
    sourceText.textContent = state.source;
  } else if (state.status === "done") {
    langLabel.textContent = `${LANG_NAMES[state.lang] ?? state.lang} → 繁體中文`;
    translationText.textContent = state.translation;
    sourceText.textContent = state.source;
  } else {
    langLabel.textContent = "無法翻譯";
    translationText.textContent = state.message;
    sourceText.textContent = "";
  }

  // 每次更新都捲回頂端，並等瀏覽器排版完成後回報內容高度給後端調整視窗大小
  window.scrollTo(0, 0);
  requestAnimationFrame(() => {
    const height = Math.min(Math.ceil(card.getBoundingClientRect().height), MAX_HEIGHT);
    invoke("fit_popup", { height });
  });
}

listen("popup-state", (event) => render(event.payload));

copyButton.addEventListener("click", async () => {
  await invoke("copy_text", { text: translationText.textContent });
  copyButton.textContent = "已複製";
});

closeButton.addEventListener("click", () => invoke("hide_popup"));

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") invoke("hide_popup");
});
