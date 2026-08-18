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
api.debugLog("popup: mounted");

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <PopupApp />
  </React.StrictMode>
);
