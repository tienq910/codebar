/** 展示格式化工具 */

/** 重置倒计时:"3d 20h" / "1h 42m" / "42m";无重置时间 → "—" */
export function fmtCountdown(resetAt: number | null | undefined, nowSec: number): string {
  if (resetAt == null) return "—";
  let s = Math.floor(resetAt - nowSec);
  if (s <= 0) return "即将重置";
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (d >= 10) return `${d}d`;
  if (d >= 1) return `${d}d ${h}h`;
  if (h >= 1) return `${h}h ${m}m`;
  return `${m}m`;
}

/** "更新于 X":刚刚 / N 分钟前 / HH:mm */
export function fmtRelative(ts: number | null, nowSec: number): string {
  if (ts == null) return "尚未刷新";
  const diff = Math.floor(nowSec - ts);
  if (diff < 50) return "刚刚";
  if (diff < 3600) return `${Math.round(diff / 60)} 分钟前`;
  const date = new Date(ts * 1000);
  const hh = String(date.getHours()).padStart(2, "0");
  const mm = String(date.getMinutes()).padStart(2, "0");
  return `${hh}:${mm}`;
}

/** 花费格式化:$1.24;null → null */
export function fmtUsd(v: number | null): string | null {
  if (v == null) return null;
  return `$${v.toFixed(2)}`;
}

/** 汇总行「最紧张窗口」:与 Rust worst_window 一致 */
export function worstWindow(windows: UsageWindowLike[]): UsageWindowLike | null {
  const withPct = windows.filter((w) => w.usedPercent != null);
  if (withPct.length > 0) {
    return withPct.reduce((a, b) => ((b.usedPercent ?? 0) > (a.usedPercent ?? 0) ? b : a));
  }
  return windows[0] ?? null;
}

interface UsageWindowLike {
  label: string;
  usedPercent: number | null;
  resetAt?: number | null;
  pace?: { text: string; hot: boolean } | null;
  note?: string | null;
}
