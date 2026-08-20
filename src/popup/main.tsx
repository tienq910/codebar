import React from "react";
import ReactDOM from "react-dom/client";
import { api } from "../shared/api";
import PopupApp from "./App";
import "../themes.css";
import "./popup.css";

// 前端异常转发到操作日志(排查真机问题)
window.addEventListener("error", (e) => {
  api.debugLog(`popup js-error: ${e.message} @${e.filename ?? ""}:${e.lineno ?? 0}`);
});
window.addEventListener("unhandledrejection", (e) => {
  api.debugLog(`popup js-rejection: ${String(e.reason)}`);
});

// 点击链路诊断(捕获阶段,先于任何 handler):
// 真机"按钮点不动"时,此日志可区分 [系统未投递点击] / [点击被吞] / [invoke 失败]
for (const type of ["pointerdown", "click"] as const) {
  document.addEventListener(
    type,
    (e) => {
      const t = e.target as HTMLElement | null;
      const desc = t
        ? `<${t.tagName.toLowerCase()}${t.className ? ` .${String(t.className).trim().split(/\s+/).join(".")}` : ""}>`
        : "null";
      api.debugLog(`popup ${type}: ${desc} @${e.clientX},${e.clientY}`);
    },
    true
  );
}
window.addEventListener("focus", () => api.debugLog("popup window focus"));
window.addEventListener("blur", () => api.debugLog("popup window blur"));
document.addEventListener("visibilitychange", () =>
  api.debugLog(`popup visibility=${document.visibilityState}`)
);

api.debugLog("popup: mounted");

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <PopupApp />
  </React.StrictMode>
);
