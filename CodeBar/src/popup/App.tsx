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

function openSettingsAt(page: "general" | "providers") {
  localStorage.setItem("codebar-settings-page", page);
  api.openSettings();
}

export default function PopupApp() {
  const [state, setState] = useState<AppState | null>(null);
  const [tab, setTab] = useState<string>(ALL);
  const [tick, setTick] = useState(0);
  const closingRef = useRef(false);

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

  // 弹窗高度自适应内容(200–560),仅 Tauri 环境
  useEffect(() => {
    if (!isTauri || !state) return;
    import("@tauri-apps/api/window").then(({ getCurrentWindow, LogicalSize }) => {
      const el = document.querySelector(".popup");
      if (!el) return;
      const h = Math.min(560, Math.max(200, el.scrollHeight));
      getCurrentWindow().setSize(new LogicalSize(372, h));
    });
  }, [state, tab, tick]);

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
              <button onClick={() => openSettingsAt("providers")}>去设置 →</button>
            </div>
            <div className="pp-foot">
              <button className="pp-act" onClick={() => openSettingsAt("general")}>
                <span className="ic">⚙</span>设置…
              </button>
              <button
                className="pp-act danger"
                onClick={() => {
                  if (closingRef.current) return;
                  closingRef.current = true;
                  api.quitApp();
                }}
              >
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
              <button className="pp-act" onClick={() => openSettingsAt("providers")}>
                <span className="ic">＋</span>添加账号 / 工具…
              </button>
              <button className="pp-act" onClick={() => openSettingsAt("general")}>
                <span className="ic">⚙</span>设置…
              </button>
              <button
                className="pp-act danger"
                onClick={() => {
                  if (closingRef.current) return;
                  closingRef.current = true;
                  api.quitApp();
                }}
              >
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
