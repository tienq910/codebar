/** 主题应用:data-theme 挂在 <html>;非法值回落 hardhacker */
import { THEME_IDS } from "./types";

export function applyTheme(theme: string) {
  const valid = (THEME_IDS as readonly string[]).includes(theme) ? theme : "hardhacker";
  document.documentElement.setAttribute("data-theme", valid);
  return valid;
}
