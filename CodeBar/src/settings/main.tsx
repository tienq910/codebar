import React from "react";
import ReactDOM from "react-dom/client";
import SettingsApp from "./App";
import "../themes.css";
import "./settings.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <SettingsApp />
  </React.StrictMode>
);
