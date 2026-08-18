import React from "react";
import ReactDOM from "react-dom/client";
import { api } from "../shared/api";
import SettingsApp from "./App";
import "../themes.css";
import "./settings.css";

// 前端异常转发到操作日志(排查真机问题)
window.addEventListener("error", (e) => {
  api.debugLog(`settings js-error: ${e.message} @${e.filename ?? ""}:${e.lineno ?? 0}`);
});
window.addEventListener("unhandledrejection", (e) => {
  api.debugLog(`settings js-rejection: ${String(e.reason)}`);
});
api.debugLog("settings: mounted");

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <SettingsApp />
  </React.StrictMode>
);
