import { useCallback, useEffect, useRef, useState } from "react";
import { api, isTauri } from "../shared/api";
import { fmtCountdown, fmtRelative, fmtUsd, worstWindow } from "../shared/format";
import { applyTheme } from "../shared/theme";
import type { AppState, ProviderSnapshot, UsageWindow } from "../shared/types";

const ALL = "__all";

function statusDotClass(s: ProviderSnapshot) {
  if (s.status.kind === "error") return "error";
  if (s.status.kind === "stale") return "stale";
  return "";
}

async function openSettingsAt(page: "general" | "providers") {
  api.debugLog(`ui:openSettings(${page})`);
  try {
    localStorage.setItem("codebar-settings-page", page);
    await api.openSettings();
    api.debugLog(`ui:openSettings(${page}) ok`);
  } catch (e) {
    api.debugLog(`ui:openSettings(${page}) FAILED: ${e}`);
  }
}

async function quitApp() {
  api.debugLog("ui:quitApp");
  try {
    await api.quitApp();
  } catch (e) {
    api.debugLog(`ui:quitApp FAILED: ${e}`);
  }
}

export default function PopupApp() {
  const [state, setState] = useState<AppState | null>(null);
  const [tab, setTab] = useState<string>(ALL);
  const [tick, setTick] = useState(0);
  const lastHeight = useRef(0);
  const dimsLogged = useRef(false);
  // 底部动作去重(pointerdown 与 click 双通道都会触发)
  const actGuard = useRef<Record<string, number>>({});
  const footerAct = (key: string, fn: () => void) => () => {
    const now = Date.now();
    if (now - (actGuard.current[key] ?? 0) < 400) return;
    actGuard.current[key] = now;
    fn();
  };
  const bindAct = (key: string, fn: () => void) => {
    const h = footerAct(key, fn);
    return { onClick: h, onPointerDown: h };
  };

  useEffect(() => {
    let alive = true;
    api.getState().then((s) => {
      if (!alive) return;
      setState(s);
      applyTheme(s.theme);
    });
    const offUsage = api.onUsageUpdated((p) => {
      setState((prev) =>
        prev ? { ...prev, snapshots: p.providers, refreshing: p.refreshing, lastRefresh: p.updatedAt } : prev
      );
    });
    const offConfig = api.onConfigUpdated((c) => {
      setState((prev) => (prev ? { ...prev, ...c } : prev));
      applyTheme(c.theme);
    });
    // 30s 一跳:驱动"更新于 X"/倒计时重算
    const timer = setInterval(() => setTick((t) => t + 1), 30_000);
    return () => {
      alive = false;
      offUsage();
      offConfig();
      clearInterval(timer);
    };
  }, []);

  const refresh = useCallback(() => {
    api.refreshNow();
  }, []);

  // 弹窗高度自适应内容:前端实测 scrollHeight → Rust 侧 set_popup_height 精调。
  // 只在窗口可见时测量(隐藏窗口布局未完成,量高不可靠);高度不变不上报,
  // 避免周期性扰动窗口。初始高度由 Rust show_popup 按空态/主界面预设。
  useEffect(() => {
    if (!isTauri || !state) return;
    const measure = () => {
      if (document.visibilityState !== "visible") return;
      const el = document.querySelector(".popup");
      if (!el) return;
      const h = Math.min(572, Math.max(212, el.scrollHeight + 12));
      if (!dimsLogged.current) {
        dimsLogged.current = true;
        api.debugLog(
          `popup dims: innerHeight=${window.innerHeight} dpr=${window.devicePixelRatio} scrollHeight=${el.scrollHeight} → h=${h}`
        );
      }
      if (h === lastHeight.current) return;
      lastHeight.current = h;
      api.setPopupHeight(h).catch((e) => api.debugLog(`ui:setPopupHeight FAILED: ${e}`));
    };
    measure();
    document.addEventListener("visibilitychange", measure);
    return () => document.removeEventListener("visibilitychange", measure);
  }, [state, tab]);

  if (!state) {
    return (
      <div className="app-shell">
        <div className="popup">
          <div className="pp-head">CODEBAR</div>
          <div style={{ padding: 20, fontSize: 12, color: "var(--dim)" }}>加载中…</div>
        </div>
      </div>
    );
  }

  const nowSec = Math.floor(Date.now() / 1000) + tick * 0; // tick 仅触发重渲染
  const connectedSnaps = state.snapshots;
  const current = connectedSnaps.find((s) => s.id === tab);

  const refreshNow = () => {
    if (!state.refreshing) refresh();
  };

  return (
    <div className="app-shell">
      <div className="popup" onMouseDown={(e) => e.stopPropagation()}>
        <div className="pp-head">
          CODEBAR
          <span className="r">
            {state.refreshing ? (
              <span className="spin" />
            ) : (
              <span style={{ fontSize: 10.5 }}>更新于 {fmtRelative(state.lastRefresh, nowSec)}</span>
            )}
            <button className="mini-btn" onClick={refreshNow} disabled={state.refreshing}>
              刷新
            </button>
          </span>
        </div>

        {connectedSnaps.length === 0 ? (
          /* ---------- 空态 ---------- */
          <>
            <div className="empty">
              <div className="big-ic">◇</div>
              <h3>还没有接入任何工具</h3>
              <p>
                额度数据会显示在这里。
                <br />
                支持自动识别、API 密钥、网页会话三种方式。
              </p>
            </div>
            <div className="setup-banner">
              <span>先接入一个工具,20 秒搞定。</span>
              <button {...bindAct("banner", () => openSettingsAt("providers"))}>去设置 →</button>
            </div>
            <div className="pp-foot">
              <button className="pp-act" {...bindAct("settings", () => openSettingsAt("general"))}>
                <span className="ic">⚙</span>设置…
              </button>
              <button className="pp-act danger" {...bindAct("quit", () => quitApp())}>
                <span className="ic">✕</span>退出 CodeBar
              </button>
            </div>
          </>
        ) : (
          /* ---------- 主界面 ---------- */
          <>
            <div className="pp-tabs">
              <button className={"pp-tab" + (tab === ALL ? " on" : "")} onClick={() => setTab(ALL)}>
                <span className="tdot" style={{ background: "var(--accent)" }}></span>汇总
              </button>
              {connectedSnaps.map((s) => (
                <button key={s.id} className={"pp-tab" + (tab === s.id ? " on" : "")} onClick={() => setTab(s.id)}>
                  <span className={"tdot " + statusDotClass(s)}></span>
                  {s.name}
                </button>
              ))}
            </div>
            <div className="pp-body" key={tab + String(state.refreshing)}>
              {tab === ALL ? (
                <SummaryView snaps={connectedSnaps} nowSec={nowSec} onOpen={setTab} />
              ) : (
                current && <DetailView snap={current} nowSec={nowSec} />
              )}
            </div>
            <div className="pp-foot">
              <button className="pp-act" {...bindAct("add", () => openSettingsAt("providers"))}>
                <span className="ic">＋</span>添加账号 / 工具…
              </button>
              <button className="pp-act" {...bindAct("settings", () => openSettingsAt("general"))}>
                <span className="ic">⚙</span>设置…
              </button>
              <button className="pp-act danger" {...bindAct("quit", () => quitApp())}>
                <span className="ic">✕</span>退出 CodeBar
              </button>
            </div>
          </>
          )}
      </div>
    </div>
  );
}

/* ---------------- 汇总视图 ---------------- */
function SummaryView({
  snaps,
  nowSec,
  onOpen,
}: {
  snaps: ProviderSnapshot[];
  nowSec: number;
  onOpen: (id: string) => void;
}) {
  const costProviders = snaps.filter((s) => s.cost?.todayUsd != null);
  const todayTotal = costProviders.reduce((sum, s) => sum + (s.cost?.todayUsd ?? 0), 0);
  return (
    <>
      {snaps.map((p) => {
        const w = worstWindow(p.windows);
        const statusMsg =
          p.status.kind === "ok" ? null : p.status.kind === "stale" ? p.status.message : p.status.message;
        return (
          <div className="ov-row" key={p.id} onClick={() => onOpen(p.id)}>
            <div className="u-title" style={{ fontWeight: 600 }}>
              {p.name}
              {p.plan && <span className="ov-plan">{p.plan}</span>}
              <span className="reset">{w ? `${fmtCountdown(w.resetAt, nowSec)} 后重置` : ""}</span>
            </div>
            {w && (
              <>
                <div className={"u-bar" + (w.pace?.hot ? " hot" : "")}>
                  <i style={{ width: `${w.usedPercent ?? 0}%` }}></i>
                </div>
                <div className="u-meta">
                  <span className="used">
                    {w.usedPercent != null ? `${w.label} · 已用 ${Math.round(w.usedPercent)}%` : (w.note ?? w.label)}
                  </span>
                  {w.pace && (
                    <span className={"pace " + (w.pace.hot ? "ahead" : "behind")}>{w.pace.text}</span>
                  )}
                </div>
              </>
            )}
            {statusMsg && (
              <div className={"ov-status" + (p.status.kind === "error" ? " error" : "")}>{statusMsg}</div>
            )}
          </div>
        );
      })}
      {costProviders.length > 0 && (
        <div className="u-block" style={{ borderBottom: "none" }}>
          <div className="u-cost">
            今日合计 <b>${todayTotal.toFixed(2)}</b>
            <span style={{ color: "var(--dim)" }}>({costProviders.length} 个计费工具)</span>
          </div>
        </div>
      )}
    </>
  );
}

/* ---------------- 详情视图 ---------------- */
function DetailView({ snap, nowSec }: { snap: ProviderSnapshot; nowSec: number }) {
  return (
    <>
      <div className="u-block">
        <div className="u-title">
          {snap.name}
          <span className="reset">{snap.plan ? `${snap.plan} 方案` : ""}</span>
        </div>
      </div>
      {snap.windows.map((w: UsageWindow) => (
        <div className="u-block" key={w.label}>
          <div className="u-title" style={{ fontWeight: 400 }}>
            {w.label}
            <span className="reset">{fmtCountdown(w.resetAt, nowSec)} 后重置</span>
          </div>
          <div className={"u-bar" + (w.pace?.hot ? " hot" : "")}>
            <i style={{ width: `${w.usedPercent ?? 0}%` }}></i>
          </div>
          <div className="u-meta">
            <span className="used">
              {w.usedPercent != null ? `已用 ${Math.round(w.usedPercent)}%` : (w.note ?? "")}
            </span>
            {w.pace && (
              <span className={"pace " + (w.pace.hot ? "ahead" : "behind")}>节奏:{w.pace.text}</span>
            )}
          </div>
        </div>
      ))}
      {snap.windows.length === 0 && (
        <div className="u-block">
          <div className="u-cost" style={{ color: "var(--dim)" }}>
            {snap.status.kind === "ok" ? "暂无窗口数据" : snap.status.message}
          </div>
        </div>
      )}
      {snap.cost && (snap.cost.todayUsd != null || snap.cost.monthUsd != null || snap.cost.note) && (
        <div className="u-block">
          <div className="u-title" style={{ fontWeight: 400 }}>
            花费
          </div>
          <div className="u-cost">
            {snap.cost.todayUsd != null && (
              <>
                今日 <b>{fmtUsd(snap.cost.todayUsd)}</b>
                {snap.cost.note ? ` · ${snap.cost.note}` : ""}
                <br />
              </>
            )}
            {snap.cost.monthUsd != null && (
              <>
                近 30 天 <b>{fmtUsd(snap.cost.monthUsd)}</b>
              </>
            )}
            {snap.cost.todayUsd == null && snap.cost.monthUsd == null && snap.cost.note && (
              <b>{snap.cost.note}</b>
            )}
          </div>
        </div>
      )}
    </>
  );
}
