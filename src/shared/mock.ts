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
      id: "kimi",
      name: "Kimi Code",
      plan: "Code",
      status: { kind: "ok" },
      windows: [mkWindow("5 小时窗口", 70, 2 * 3600 + 11 * 60, 5 * 3600), mkWindow("每周窗口", 20, 6 * 86400, 7 * 86400)],
      cost: null,
      updatedAt: now() - 90,
    },
    {
      id: "zai",
      name: "z.ai / GLM",
      plan: "Max",
      status: { kind: "ok" },
      windows: [mkWindow("5 小时窗口", 42, 4 * 3600 + 30 * 60, 5 * 3600), mkWindow("每周窗口", 30, 5 * 86400, 7 * 86400)],
      cost: null,
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
  ];
}

/** 与 Rust providers/matrix.rs 同步的完整矩阵(id, 名称, 认证, 凭据来源);顺序一致:deepseek/codex/kimi/zai 置顶 */
const MATRIX: Array<[string, string, "auto" | "key" | "cookie", string]> = [
  ["deepseek", "DeepSeek", "key", "API Key(DEEPSEEK_API_KEY)"],
  ["codex", "Codex", "auto", "~/.codex/auth.json"],
  ["kimi", "Kimi Code", "key", "API Key(KIMI_CODE_API_KEY)"],
  ["zai", "z.ai / GLM", "key", "API Key(Z_AI_API_KEY)"],
  ["claude", "Claude", "auto", "Claude Code CLI"],
  ["openai", "OpenAI", "key", "Admin API Key"],
  ["gemini", "Gemini", "auto", "Gemini CLI OAuth"],
  ["cursor", "Cursor", "cookie", "浏览器会话 Cookie"],
  ["azureopenai", "Azure OpenAI", "key", "API Key + Endpoint"],
  ["clinepass", "ClinePass", "key", "API Key(CLINE_API_KEY)"],
  ["opencode", "OpenCode", "auto", "~/.local/share/opencode/auth.json"],
  ["opencodego", "OpenCode Go", "auto", "~/.local/share/opencode/auth.json"],
  ["alibaba", "Alibaba Coding Plan", "key", "API Key(DASHSCOPE_API_KEY)"],
  ["alibabatokenplan", "Alibaba Token Plan", "cookie", "浏览器会话 Cookie"],
  ["qwencloud", "Qwen Cloud", "cookie", "浏览器会话 Cookie"],
  ["factory", "Droid", "key", "API Key(FACTORY_API_KEY)"],
  ["fireworks", "Fireworks", "key", "API Key(FIREWORKS_API_KEY)"],
  ["antigravity", "Antigravity", "auto", "Antigravity OAuth 凭据"],
  ["copilot", "Copilot", "key", "Copilot API Token"],
  ["devin", "Devin", "cookie", "浏览器会话 Cookie"],
  ["minimax", "MiniMax", "key", "API Key(MINIMAX_API_KEY)"],
  ["manus", "Manus", "cookie", "浏览器会话 Cookie"],
  ["kilo", "Kilo", "auto", "~/.local/share/kilo/auth.json"],
  ["kiro", "Kiro", "auto", "Kiro CLI 会话"],
  ["vertexai", "Vertex AI", "auto", "gcloud ADC 凭据"],
  ["augment", "Augment", "auto", "Augment CLI 会话"],
  ["jetbrains", "JetBrains AI", "auto", "本地 IDE 配置"],
  ["moonshot", "Moonshot", "key", "API Key(MOONSHOT_API_KEY)"],
  ["amp", "Amp", "key", "API Key(AMP_API_KEY)"],
  ["t3chat", "T3 Chat", "cookie", "浏览器会话 Cookie"],
  ["ollama", "Ollama", "auto", "本地服务(OLLAMA_HOST:11434)"],
  ["openrouter", "OpenRouter", "key", "API Key(OPENROUTER_API_KEY)"],
  ["elevenlabs", "ElevenLabs", "key", "API Key(ELEVENLABS_API_KEY)"],
  ["warp", "Warp", "key", "API Key(WARP_API_KEY)"],
  ["windsurf", "Windsurf", "cookie", "浏览器会话 Cookie"],
  ["zed", "Zed", "auto", "~/.config/zed/settings.json"],
  ["perplexity", "Perplexity", "cookie", "浏览器会话 Cookie"],
  ["mimo", "Xiaomi MiMo", "cookie", "浏览器会话 Cookie"],
  ["doubao", "Doubao", "key", "API Key(ARK_API_KEY)"],
  ["sakana", "Sakana AI", "cookie", "浏览器会话 Cookie"],
  ["abacus", "Abacus AI", "cookie", "浏览器会话 Cookie"],
  ["mistral", "Mistral", "cookie", "浏览器会话 Cookie"],
  ["deepinfra", "DeepInfra", "key", "API Key(DEEPINFRA_API_KEY)"],
  ["codebuff", "Codebuff", "key", "API Key(CODEBUFF_API_KEY)"],
  ["crof", "Crof", "key", "API Key(CROF_API_KEY)"],
  ["venice", "Venice", "key", "API Key(VENICE_API_KEY)"],
  ["commandcode", "Command Code", "cookie", "浏览器会话 Cookie"],
  ["qoder", "Qoder", "cookie", "浏览器会话 Cookie"],
  ["stepfun", "StepFun", "key", "Token(STEPFUN_TOKEN)"],
  ["bedrock", "AWS Bedrock", "key", "Access Key ID + Secret"],
  ["grok", "Grok", "auto", "~/.grok/auth.json"],
  ["groq", "Groq", "key", "API Key(GROQ_API_KEY)"],
  ["llmproxy", "LLM Proxy", "key", "API Key(LLM_PROXY_API_KEY)"],
  ["litellm", "LiteLLM", "key", "API Key(LITELLM_API_KEY)"],
  ["deepgram", "Deepgram", "key", "API Key(DEEPGRAM_API_KEY)"],
  ["poe", "Poe", "key", "API Key(POE_API_KEY)"],
  ["chutes", "Chutes", "key", "API Key(CHUTES_API_KEY)"],
  ["neuralwatt", "Neuralwatt", "key", "API Key(NEURALWATT_API_KEY)"],
  ["clawrouter", "ClawRouter", "key", "API Key(CLAWROUTER_API_KEY)"],
  ["longcat", "LongCat", "cookie", "手动 Cookie 头"],
  ["sub2api", "sub2api", "key", "API Key(SUB2API_API_KEY)"],
  ["wayfinder", "Wayfinder", "key", "网关 URL(免认证)"],
  ["zenmux", "ZenMux", "key", "API Key(ZENMUX_MANAGEMENT_API_KEY)"],
  ["aiand", "ai&", "key", "API Key(AIAND_API_KEY)"],
  ["zoommate", "ZoomMate", "cookie", "浏览器会话 Cookie"],
  ["xai", "xAI", "key", "API Key(XAI_MANAGEMENT_API_KEY)"],
  ["notion", "Notion AI", "cookie", "浏览器会话 Cookie"],
  ["ibmbob", "IBM Bob", "key", "API Key(BOBSHELL_API_KEY)"],
];

const PROVIDERS: ProviderDescriptor[] = MATRIX.map(([id, name, auth, hint]) => ({ id, name, auth, hint }));

type Listener = (payload: unknown) => void;

class MockBackend {
  theme = localStorage.getItem("codebar-theme") || "hardhacker";
  refreshInterval = "adaptive";
  autostart = false;
  connected: string[] = ["deepseek", "codex", "kimi", "zai", "claude"];
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

  refreshNow() {
    if (this.refreshing) return;
    this.refreshing = true;
    this.emitUsage();
    setTimeout(() => {
      this.refreshing = false;
      this.lastRefresh = now();
      this.snapshots = mockSnapshots()
        .filter((s) => this.connected.includes(s.id))
        .map((s) => ({ ...s, updatedAt: now() }));
      this.emitUsage();
    }, 1400);
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
    if (desc.auth === "key" && cred.length < 16) throw new Error("密钥长度过短");
    if (desc.auth === "cookie" && !cred.includes("=")) throw new Error("需要合法的 Cookie 头");
    await new Promise((r) => setTimeout(r, 1000));
    if (cred.toLowerCase().includes("bad")) throw new Error("401 — 服务端拒绝该密钥");
    this.connected.push(id);
    this.snapshots = mockSnapshots().filter((s) => this.connected.includes(s.id));
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
