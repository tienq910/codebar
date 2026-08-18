/** 与 Rust 侧 models/config 对齐的类型(serde camelCase) */

export type AuthKind = "auto" | "key" | "cookie";

export interface ProviderDescriptor {
  id: string;
  name: string;
  auth: AuthKind;
  hint: string;
}

export type ProviderStatus =
  | { kind: "ok" }
  | { kind: "stale"; message: string }
  | { kind: "error"; message: string };

export interface PaceInfo {
  text: string;
  hot: boolean;
}

export interface UsageWindow {
  label: string;
  usedPercent: number | null;
  resetAt: number | null;
  windowSeconds: number | null;
  note: string | null;
  pace: PaceInfo | null;
}

export interface Cost {
  todayUsd: number | null;
  monthUsd: number | null;
  note: string | null;
}

export interface ProviderSnapshot {
  id: string;
  name: string;
  plan: string | null;
  status: ProviderStatus;
  windows: UsageWindow[];
  cost: Cost | null;
  updatedAt: number;
}

export interface ScanResult {
  found: boolean;
  path: string | null;
  valid: boolean;
}

export interface AppState {
  theme: string;
  refreshInterval: string;
  autostart: boolean;
  connected: string[];
  providers: ProviderDescriptor[];
  snapshots: ProviderSnapshot[];
  refreshing: boolean;
  lastRefresh: number | null;
  dataDir: string;
  version: string;
}

export interface UsageUpdatedPayload {
  providers: ProviderSnapshot[];
  updatedAt: number;
  refreshing: boolean;
}

export interface ConfigUpdatedPayload {
  theme: string;
  refreshInterval: string;
  autostart: boolean;
  connected: string[];
}

export const THEMES = [
  { id: "hardhacker", name: "Hard Hacker", dots: ["#282433", "#e965a5", "#b1f2a7"] },
  { id: "mocha", name: "Mocha", dots: ["#1e1e2e", "#cba6f7", "#89b4fa"] },
  { id: "latte", name: "Latte", dots: ["#eff1f5", "#8839ef", "#1e66f5"] },
] as const;

export const THEME_IDS = THEMES.map((t) => t.id) as readonly string[];

export const AUTH_CN: Record<AuthKind, string> = {
  auto: "自动识别",
  key: "API 密钥",
  cookie: "网页会话",
};

export const REFRESH_OPTIONS = [
  { id: "adaptive", name: "自适应(默认)" },
  { id: "manual", name: "手动" },
  { id: "1m", name: "每 1 分钟" },
  { id: "2m", name: "每 2 分钟" },
  { id: "5m", name: "每 5 分钟" },
  { id: "15m", name: "每 15 分钟" },
  { id: "30m", name: "每 30 分钟" },
] as const;
