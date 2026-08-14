/**
 * 浏览器预览 mock(npm run dev 无 Tauri 时启用):
 * 完整模拟 Rust 命令与事件行为,让 popup/settings 两页可在纯浏览器里开发与走查。
 * 主题持久化到 localStorage;数据参照原型的假数据。
 */
import type {
  AppState,
  ConfigUpdatedPayload,
  ProviderDescriptor,
  ProviderSnapshot,
  ScanResult,
  UsageUpdatedPayload,
} from "./types";

const now = () => Math.floor(Date.now() / 1000);

function windowPace(usedPercent: number, elapsedFrac: number) {
  const delta = usedPercent - elapsedFrac * 100;
  if (Math.abs(delta) < 2) return { text: "持平", hot: false };
  return delta > 0
    ? { text: `超前 +${Math.round(delta)}%`, hot: true }
    : { text: `落后 -${Math.round(Math.abs(delta))}%`, hot: false };
}

function mkWindow(label: string, used: number, resetInSec: number, windowSec: number) {
  const w = {
    label,
    usedPercent: used,
    resetAt: now() + resetInSec,
    windowSeconds: windowSec,
    note: null,
    pace: { text: "", hot: false },
  };
  w.pace = windowPace(used, 1 - resetInSec / windowSec);
  return w;
}

function mockSnapshots(): ProviderSnapshot[] {
  return [
    {
      id: "codex",
      name: "Codex",
      plan: "Plus",
      status: { kind: "ok" },
      windows: [mkWindow("5 小时窗口", 68, 3600 + 42 * 60, 5 * 3600), mkWindow("每周窗口", 34, 3 * 86400 + 20 * 3600, 7 * 86400)],
      cost: { todayUsd: 1.24, monthUsd: 41.07, note: "86K tokens" },
      updatedAt: now() - 90,
    },
    {
      id: "claude",
      name: "Claude",
      plan: "Max",
      status: { kind: "ok" },
      windows: [mkWindow("5 小时窗口", 8, 3 * 3600 + 53 * 60, 5 * 3600), mkWindow("每周窗口", 15, 3 * 86400 + 20 * 3600, 7 * 86400)],
      cost: { todayUsd: 0.04, monthUsd: 254.24, note: "15K tokens" },
      updatedAt: now() - 90,
    },
    {
      id: "deepseek",
      name: "DeepSeek",
      plan: "按量",
      status: { kind: "ok" },
      windows: [
        { label: "余额", usedPercent: null, resetAt: null, windowSeconds: null, note: "余额 ¥110.00", pace: null },
      ],
      cost: null,
      updatedAt: now() - 90,
    },
  ];
}

const PROVIDERS: ProviderDescriptor[] = [
  { id: "codex", name: "Codex", auth: "auto", hint: "~/.codex/auth.json" },
  { id: "claude", name: "Claude", auth: "auto", hint: "Claude Code CLI" },
  { id: "gemini", name: "Gemini", auth: "auto", hint: "Gemini CLI OAuth" },
  { id: "openai", name: "OpenAI", auth: "key", hint: "Admin API Key" },
  { id: "deepseek", name: "DeepSeek", auth: "key", hint: "API Key" },
  { id: "cursor", name: "Cursor", auth: "cookie", hint: "浏览器会话 Cookie" },
];

type Listener = (payload: unknown) => void;

class MockBackend {
  theme = localStorage.getItem("codebar-theme") || "hardhacker";
  refreshInterval = "adaptive";
  autostart = false;
  connected: string[] = ["codex", "claude", "deepseek"];
  refreshing = false;
  lastRefresh: number | null = now() - 90;
  snapshots: ProviderSnapshot[] = mockSnapshots();
  private usageListeners = new Set<Listener>();
  private configListeners = new Set<Listener>();

  onUsage(cb: Listener) {
    this.usageListeners.add(cb);
    return () => this.usageListeners.delete(cb);
  }
  onConfig(cb: Listener) {
    this.configListeners.add(cb);
    return () => this.configListeners.delete(cb);
  }
  private emitUsage() {
    const payload: UsageUpdatedPayload = {
      providers: this.snapshots,
      updatedAt: this.lastRefresh ?? now(),
      refreshing: this.refreshing,
    };
    this.usageListeners.forEach((cb) => cb(payload));
  }
  private emitConfig() {
    const payload: ConfigUpdatedPayload = {
      theme: this.theme,
      refreshInterval: this.refreshInterval,
      autostart: this.autostart,
      connected: this.connected,
    };
    this.configListeners.forEach((cb) => cb(payload));
  }

  getState(): AppState {
    return {
      theme: this.theme,
      refreshInterval: this.refreshInterval,
      autostart: this.autostart,
      connected: this.connected,
      providers: PROVIDERS,
      snapshots: this.snapshots,
      refreshing: this.refreshing,
      lastRefresh: this.lastRefresh,
      dataDir: "D:\\CodeBar\\data(预览 mock)",
      version: "0.1.0-dev",
    };
  }

  async refreshNow() {
    if (this.refreshing) return;
    this.refreshing = true;
    this.emitUsage();
    await new Promise((r) => setTimeout(r, 1400));
    this.refreshing = false;
    this.lastRefresh = now();
    this.snapshots = mockSnapshots().map((s) =>
      this.connected.includes(s.id) ? { ...s, updatedAt: now() } : s
    );
    this.emitUsage();
  }

  async scanCli(id: string): Promise<ScanResult> {
    await new Promise((r) => setTimeout(r, 1200));
    if (id === "codex") return { found: true, path: "/Users/demo/.codex/auth.json", valid: true };
    if (id === "claude") return { found: true, path: "/Users/demo/.claude/.credentials.json", valid: true };
    if (id === "gemini") return { found: false, path: "/Users/demo/.gemini/oauth_creds.json", valid: false };
    return { found: false, path: null, valid: false };
  }

  async connectProvider(id: string, credential?: string) {
    const desc = PROVIDERS.find((p) => p.id === id);
    if (!desc) throw new Error("未知 provider");
    if (desc.auth === "auto") {
      const scan = await this.scanCli(id);
      if (scan.found && scan.valid) {
        this.connected.push(id);
        this.emitConfig();
        return;
      }
      throw new Error("未找到本机凭据");
    }
    const cred = (credential ?? "").trim();
    if (desc.auth === "key" && cred.length < 20) throw new Error("密钥长度过短");
    if (desc.auth === "cookie" && !cred.includes("=")) throw new Error("需要合法的 Cookie 头");
    await new Promise((r) => setTimeout(r, 1000));
    if (cred.toLowerCase().includes("bad")) throw new Error("401 — 服务端拒绝该密钥");
    this.connected.push(id);
    if (id === "deepseek" && !this.snapshots.find((s) => s.id === "deepseek")) {
      this.snapshots = [...this.snapshots, ...mockSnapshots().filter((s) => s.id === "deepseek")];
    }
    this.emitConfig();
    this.emitUsage();
  }

  disconnectProvider(id: string) {
    this.connected = this.connected.filter((c) => c !== id);
    this.snapshots = this.snapshots.filter((s) => s.id !== id);
    this.emitConfig();
    this.emitUsage();
  }

  setTheme(theme: string) {
    this.theme = theme;
    localStorage.setItem("codebar-theme", theme);
    this.emitConfig();
  }
  setRefreshInterval(interval: string) {
    this.refreshInterval = interval;
    this.emitConfig();
  }
  setAutostart(enabled: boolean) {
    this.autostart = enabled;
    this.emitConfig();
  }
  openSettings() {
    /* 浏览器预览:两页并排打开,无窗口管理 */
  }
  quitApp() {
    console.info("[mock] quitApp");
  }
}

export const mock = new MockBackend();
