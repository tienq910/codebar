import { useEffect, useState } from "react";
import { api, isTauri } from "../shared/api";
import { applyTheme } from "../shared/theme";
import { AUTH_CN, ProviderDescriptor, REFRESH_OPTIONS, THEMES } from "../shared/types";

type Page = "general" | "providers" | "display" | "about";
type Phase = "idle" | "scanning" | "input" | "checking" | "ok" | "missing" | "fail";

const PAGE_NAMES: Record<Page, string> = {
  general: "常规",
  providers: "Providers",
  display: "显示",
  about: "关于",
};

const REPO_URL = "https://github.com/tienq910/codebar";

export default function SettingsApp() {
  const [page, setPage] = useState<Page>(() => {
    // 优先 URL ?page=,其次 localStorage 提示(弹窗"添加账号"等入口),默认常规
    const fromUrl = new URLSearchParams(window.location.search).get("page");
    const hint = fromUrl ?? localStorage.getItem("codebar-settings-page");
    localStorage.removeItem("codebar-settings-page");
    return (["general", "providers", "display", "about"] as const).includes(hint as Page)
      ? (hint as Page)
      : "general";
  });
  const [theme, setTheme] = useState("hardhacker");
  const [interval, setInterval_] = useState("adaptive");
  const [autostart, setAutostart] = useState(false);
  const [version, setVersion] = useState("");
  const [dataDir, setDataDir] = useState("");
  const [providers, setProviders] = useState<ProviderDescriptor[]>([]);
  const [connected, setConnected] = useState<string[]>([]);
  const [flash, setFlash] = useState("");

  useEffect(() => {
    api.getState().then((s) => {
      setTheme(applyTheme(s.theme));
      setInterval_(s.refreshInterval);
      setAutostart(s.autostart);
      setVersion(s.version);
      setDataDir(s.dataDir);
      setProviders(s.providers);
      setConnected(s.connected);
    });
    return api.onConfigUpdated((c) => {
      setTheme(applyTheme(c.theme));
      setInterval_(c.refreshInterval);
      setAutostart(c.autostart);
      setConnected(c.connected);
    });
  }, []);

  const changeTheme = (id: string) => {
    setTheme(applyTheme(id));
    api.setTheme(id);
  };
  const changeInterval = (id: string) => {
    setInterval_(id);
    api.setRefreshInterval(id);
  };
  const changeAutostart = (on: boolean) => {
    setAutostart(on);
    api.setAutostart(on);
  };
  const triggerRefresh = async () => {
    await api.refreshNow();
    setFlash("已触发刷新,结果将推送到托盘弹窗");
    setTimeout(() => setFlash(""), 2500);
  };
  const closeWindow = () => {
    if (isTauri) {
      import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow().hide());
    }
  };

  return (
    <div className="settings-win">
      <div className="sw-title" data-tauri-drag-region>
        设置
        <button className="x" onClick={closeWindow} title="关闭">
          ✕
        </button>
      </div>
      <div className="sw-body">
        <div className="sw-nav">
          {(Object.keys(PAGE_NAMES) as Page[]).map((p) => (
            <button key={p} className={page === p ? "on" : ""} onClick={() => setPage(p)}>
              {PAGE_NAMES[p]}
            </button>
          ))}
        </div>
        <div className="sw-list" key={page}>
          {page === "general" && (
            <GeneralPage
              autostart={autostart}
              onAutostart={changeAutostart}
              interval={interval}
              onInterval={changeInterval}
              onRefresh={triggerRefresh}
              flash={flash}
              dataDir={dataDir}
              onOpenLogDir={() => api.openLogDir()}
            />
          )}
          {page === "providers" && (
            <ProvidersPage providers={providers} connected={connected} onChanged={setConnected} />
          )}
          {page === "display" && <DisplayPage theme={theme} onTheme={changeTheme} />}
          {page === "about" && <AboutPage version={version} dataDir={dataDir} />}
        </div>
      </div>
    </div>
  );
}

/* ---------------- 常规 ---------------- */
function GeneralPage(props: {
  autostart: boolean;
  onAutostart: (v: boolean) => void;
  interval: string;
  onInterval: (v: string) => void;
  onRefresh: () => void;
  flash: string;
  dataDir: string;
  onOpenLogDir: () => void;
}) {
  return (
    <>
      <div className="pg-title">常规</div>
      <div className="pg-card">
        <div className="pg-row">
          <div>
            <div className="pg-label">开机自启</div>
            <div className="pg-desc">默认关闭;开启时写入注册表 Run 键(唯一的系统写入)</div>
          </div>
          <div className="right">
            <label className="switch">
              <input
                type="checkbox"
                checked={props.autostart}
                onChange={(e) => props.onAutostart(e.target.checked)}
              />
              <span className="track" />
            </label>
          </div>
        </div>
      </div>
      <div className="pg-card">
        <div className="pg-row">
          <div>
            <div className="pg-label">刷新间隔</div>
            <div className="pg-desc">自适应:近期有交互 2m、1h 内 5m、1–4h 15m、更久 30m</div>
          </div>
          <div className="right">
            <select className="pg-select" value={props.interval} onChange={(e) => props.onInterval(e.target.value)}>
              {REFRESH_OPTIONS.map((o) => (
                <option key={o.id} value={o.id}>
                  {o.name}
                </option>
              ))}
            </select>
          </div>
        </div>
        <div className="pg-row">
          <div>
            <div className="pg-label">手动刷新始终可用</div>
            <div className="pg-desc">同一时刻只允许一批 provider 刷新</div>
          </div>
          <div className="right">
            <button className="pv-btn" onClick={props.onRefresh}>
              立即刷新
            </button>
          </div>
        </div>
        {props.flash && (
          <div className="pg-row">
            <div className="pg-desc" style={{ color: "var(--ok)" }}>
              {props.flash}
            </div>
          </div>
        )}
      </div>
      <div className="pg-card">
        <div className="pg-row">
          <div>
            <div className="pg-label">数据目录(便携)</div>
            <div className="pg-desc">所有配置与密钥保存在 exe 同级 data/,不写 %APPDATA% 与注册表</div>
          </div>
        </div>
        <div className="pg-row">
          <div className="pg-desc mono" style={{ userSelect: "text" }}>
            {props.dataDir || "…"}
          </div>
        </div>
      </div>
      <div className="pg-card">
        <div className="pg-row">
          <div>
            <div className="pg-label">诊断日志</div>
            <div className="pg-desc">
              记录启动/刷新/接入/弹窗操作与错误,位于 data/codebar.log
              <br />
              超过 512KB 自动轮转为 codebar.log.old;绝不记录密钥内容
            </div>
          </div>
          <div className="right">
            <button className="pv-btn" onClick={props.onOpenLogDir}>
              打开日志目录
            </button>
          </div>
        </div>
      </div>
    </>
  );
}

/* ---------------- Providers(三种行内认证链路) ---------------- */
type ScanInfo = { path: string | null };

function ProvidersPage(props: {
  providers: ProviderDescriptor[];
  connected: string[];
  onChanged: (connected: string[]) => void;
}) {
  const [openId, setOpenId] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>("idle");
  const [val, setVal] = useState("");
  const [err, setErr] = useState("");
  const [scan, setScan] = useState<ScanInfo>({ path: null });
  const [filter, setFilter] = useState("");

  const visibleProviders = filter.trim()
    ? props.providers.filter((p) => {
        const f = filter.trim().toLowerCase();
        return p.name.toLowerCase().includes(f) || p.id.includes(f);
      })
    : props.providers;

  const reset = () => {
    setOpenId(null);
    setPhase("idle");
    setVal("");
    setErr("");
  };

  const openAuth = (p: ProviderDescriptor) => {
    setOpenId(p.id);
    setVal("");
    setErr("");
    if (p.auth === "auto") {
      setPhase("scanning");
      runScan(p.id);
    } else {
      setPhase("input");
    }
  };

  const runScan = async (id: string) => {
    setPhase("scanning");
    try {
      const r = await api.scanCli(id);
      setScan({ path: r.path });
      if (r.found && r.valid) {
        // 凭据有效 → 接入(后端复扫并写入 config)
        await api.connectProvider(id);
        setPhase("ok");
        setTimeout(() => {
          refreshConnected();
          reset();
        }, 800);
      } else if (r.found) {
        setPhase("fail");
        setErr("找到凭据文件但内容不可用,请重新登录对应 CLI");
      } else {
        setPhase("missing");
      }
    } catch (e) {
      setPhase("fail");
      setErr(String(e));
    }
  };

  const refreshConnected = async () => {
    const s = await api.getState();
    props.onChanged(s.connected);
  };

  const submit = async (p: ProviderDescriptor) => {
    const v = val.trim();
    if (p.auth === "key" && v.length < 20) {
      setErr("密钥长度过短");
      setPhase("input");
      return;
    }
    if (p.auth === "cookie" && (!v.includes("=") || v.length < 10)) {
      setErr("需要合法的 Cookie 头(形如 session=…; …)");
      setPhase("input");
      return;
    }
    setPhase("checking");
    try {
      await api.connectProvider(p.id, v);
      setPhase("ok");
      setTimeout(() => {
        refreshConnected();
        reset();
      }, 700);
    } catch (e) {
      setPhase("input");
      setErr(e instanceof Error ? e.message : String(e));
    }
  };

  const disconnect = async (id: string) => {
    await api.disconnectProvider(id);
    refreshConnected();
  };

  return (
    <>
      <div className="pg-title" style={{ display: "flex", alignItems: "center", gap: 10 }}>
        Providers
        <span style={{ fontWeight: 400, fontSize: 11, color: "var(--dim)" }}>
          {props.providers.length} 个
        </span>
        <input
          className="pv-input"
          style={{ margin: 0, marginLeft: "auto", width: 150, padding: "5px 10px", fontSize: 11.5 }}
          placeholder="搜索…"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
      </div>
      {visibleProviders.map((p) => {
        const isLinked = props.connected.includes(p.id);
        const isOpen = openId === p.id;
        return (
          <div key={p.id} className={"pv-row" + (isLinked ? " linked" : "")}>
            <div className="pv-main">
              <div>
                <div className="pv-name">{p.name}</div>
                <div className="pv-sub">
                  {AUTH_CN[p.auth]} · {p.hint}
                </div>
              </div>
              <div className="pv-state">
                {isLinked && <span className="linked-t">● 已接入</span>}
                {isLinked && (
                  <button
                    className="pv-btn danger"
                    onClick={() => disconnect(p.id)}
                    title="断开并清除本工具密钥"
                  >
                    断开
                  </button>
                )}
                {!isLinked && !isOpen && (
                  <button className="pv-btn" onClick={() => openAuth(p)}>
                    接入
                  </button>
                )}
                {!isLinked && isOpen && (
                  <button className="pv-btn" onClick={reset}>
                    收起
                  </button>
                )}
              </div>
            </div>
            {isOpen && (
              <div className="pv-auth-zone">
                {p.auth === "auto" && (
                  <>
                    <div className="how">读取本机 {p.hint},复用已登录凭据。</div>
                    {phase === "scanning" && (
                      <div className="scan-row">
                        <span className="spin"></span>
                        <span>scan {(scan.path ?? p.hint).replace(/^~/, "~")}</span>
                        <button className="pv-btn" style={{ marginLeft: "auto" }} onClick={reset}>
                          跳过
                        </button>
                      </div>
                    )}
                    {phase === "ok" && (
                      <div className="scan-row">
                        <span className="ok-t">✓ 凭据有效,已接入</span>
                      </div>
                    )}
                    {phase === "fail" && (
                      <>
                        <div className="scan-row">
                          <span className="err-t">✕ {err || "扫描失败"}</span>
                        </div>
                        <div className="pv-actions">
                          <button className="pv-btn" onClick={() => runScan(p.id)}>
                            重新扫描
                          </button>
                          <button className="pv-btn" onClick={reset}>
                            关闭
                          </button>
                        </div>
                      </>
                    )}
                    {phase === "missing" && (
                      <>
                        <div className="scan-row">
                          <span className="err-t">✕ 未找到本机凭据{scan.path ? `(${scan.path})` : ""}</span>
                        </div>
                        <div className="pv-actions">
                          <button className="pv-btn" onClick={() => runScan(p.id)}>
                            重新扫描
                          </button>
                          <button className="pv-btn" onClick={reset}>
                            关闭
                          </button>
                        </div>
                      </>
                    )}
                  </>
                )}
                {p.auth === "key" && (
                  <>
                    <div className="how">粘贴 {p.hint},验证后 DPAPI 加密存储到 data/secrets.bin。</div>
                    <input
                      className={"pv-input" + (err ? " bad" : "")}
                      placeholder="sk-…"
                      value={val}
                      disabled={phase === "checking" || phase === "ok"}
                      onChange={(e) => {
                        setVal(e.target.value);
                        setErr("");
                      }}
                      onKeyDown={(e) => e.key === "Enter" && submit(p)}
                    />
                    <div className="pv-err">{phase === "ok" ? "✓ 验证通过,已接入" : err}</div>
                    <div className="pv-actions">
                      <button className="pv-btn" disabled={phase === "checking" || phase === "ok"} onClick={() => submit(p)}>
                        {phase === "checking" ? "验证中…" : "验证并接入"}
                      </button>
                      <button className="pv-btn" onClick={reset}>
                        取消
                      </button>
                    </div>
                  </>
                )}
                {p.auth === "cookie" && (
                  <>
                    <div className="how">
                      ① 登录 {p.name} 网页版 → ② 按 F12 打开开发者工具,Network 任选一个请求 → ③ 复制请求头里的
                      Cookie 值粘贴到下面,校验后 DPAPI 加密存储。
                    </div>
                    <input
                      className={"pv-input" + (err ? " bad" : "")}
                      placeholder="session=…; …"
                      value={val}
                      disabled={phase === "checking" || phase === "ok"}
                      onChange={(e) => {
                        setVal(e.target.value);
                        setErr("");
                      }}
                      onKeyDown={(e) => e.key === "Enter" && submit(p)}
                    />
                    <div className="pv-err">{phase === "ok" ? "✓ 校验通过,已接入" : err}</div>
                    <div className="pv-actions">
                      <button className="pv-btn" disabled={phase === "checking" || phase === "ok"} onClick={() => submit(p)}>
                        {phase === "checking" ? "校验中…" : "校验并接入"}
                      </button>
                      <button className="pv-btn" onClick={reset}>
                        取消
                      </button>
                    </div>
                  </>
                )}
              </div>
            )}
          </div>
        );
      })}
    </>
  );
}

/* ---------------- 显示 ---------------- */
function DisplayPage(props: { theme: string; onTheme: (id: string) => void }) {
  return (
    <>
      <div className="pg-title">显示</div>
      <div className="pg-card">
        <div className="pg-row" style={{ marginBottom: 10 }}>
          <div>
            <div className="pg-label">主题</div>
            <div className="pg-desc">切换即时生效(0.5s 过渡)并持久化;非法值回落 Hard Hacker</div>
          </div>
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {THEMES.map((t) => (
            <button key={t.id} className={"swatch" + (props.theme === t.id ? " on" : "")} onClick={() => props.onTheme(t.id)}>
              <span className="dots">
                {t.dots.map((c) => (
                  <i key={c} style={{ background: c }}></i>
                ))}
              </span>
              {t.name}
              <span className="ck">✓</span>
            </button>
          ))}
        </div>
      </div>
    </>
  );
}

/* ---------------- 关于 ---------------- */
function AboutPage(props: { version: string; dataDir: string }) {
  const openRepo = () => {
    if (isTauri) {
      import("@tauri-apps/plugin-opener").then((m) => m.openUrl(REPO_URL));
    } else {
      window.open(REPO_URL, "_blank");
    }
  };
  return (
    <>
      <div className="pg-title">关于</div>
      <div className="pg-card">
        <div className="about-logo">
          <span className="bars">
            <i></i>
            <i></i>
            <i></i>
          </span>
          CodeBar
        </div>
        <div className="about-kv">
          版本 <b>v{props.version || "0.1.0"}</b> · Windows 10 系统托盘应用
          <br />
          AI 编程工具额度用量 · 重置倒计时 · 花费
        </div>
      </div>
      <div className="pg-card">
        <div className="pg-row">
          <div>
            <div className="pg-label">便携版说明</div>
            <div className="pg-desc">
              免安装,解压即用,可放 U 盘;所有数据在 exe 同级 data/ 目录。
              <br />
              密钥经 DPAPI 加密,拷贝到其他机器后自动失效(安全特性)。
              <br />
              唯一依赖:WebView2 Runtime(Win10/11 一般自带)。
            </div>
          </div>
        </div>
      </div>
      <div className="pg-card">
        <div className="pg-row">
          <div className="about-kv">
            源码与发布:<span className="link" onClick={openRepo}>{REPO_URL.replace("https://", "")}</span>
          </div>
        </div>
      </div>
    </>
  );
}
