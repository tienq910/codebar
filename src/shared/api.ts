/** invoke 封装:Tauri 环境走真实命令,浏览器走 mock(预览用) */
import type { AppState, ConfigUpdatedPayload, ScanResult, UsageUpdatedPayload } from "./types";
import { mock } from "./mock";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export const api = {
  getState(): Promise<AppState> {
    if (isTauri) return invoke<AppState>("get_state");
    return Promise.resolve(mock.getState());
  },
  refreshNow(): Promise<void> {
    if (isTauri) return invoke<void>("refresh_now");
    mock.refreshNow();
    return Promise.resolve();
  },
  scanCli(id: string): Promise<ScanResult> {
    if (isTauri) return invoke<ScanResult>("scan_cli", { id });
    return mock.scanCli(id);
  },
  connectProvider(id: string, credential?: string): Promise<void> {
    if (isTauri) return invoke<void>("connect_provider", { id, credential: credential ?? null });
    return mock.connectProvider(id, credential);
  },
  disconnectProvider(id: string): Promise<void> {
    if (isTauri) return invoke<void>("disconnect_provider", { id });
    mock.disconnectProvider(id);
    return Promise.resolve();
  },
  setTheme(theme: string): Promise<void> {
    if (isTauri) return invoke<void>("set_theme", { theme });
    mock.setTheme(theme);
    return Promise.resolve();
  },
  setRefreshInterval(interval: string): Promise<void> {
    if (isTauri) return invoke<void>("set_refresh_interval", { interval });
    mock.setRefreshInterval(interval);
    return Promise.resolve();
  },
  setAutostart(enabled: boolean): Promise<void> {
    if (isTauri) return invoke<void>("set_autostart", { enabled });
    mock.setAutostart(enabled);
    return Promise.resolve();
  },
  openSettings(): Promise<void> {
    if (isTauri) return invoke<void>("open_settings");
    mock.openSettings();
    return Promise.resolve();
  },
  /** 实测内容高度 → Rust 精调弹窗高度(仅窗口可见时生效) */
  setPopupHeight(height: number): Promise<void> {
    if (isTauri) return invoke<void>("set_popup_height", { height });
    return Promise.resolve();
  },
  quitApp(): Promise<void> {
    if (isTauri) return invoke<void>("quit_app");
    mock.quitApp();
    return Promise.resolve();
  },
  /** 诊断日志 → data/codebar.log(浏览器预览打印到 console) */
  debugLog(message: string): void {
    if (isTauri) {
      invoke<void>("debug_log", { message }).catch(() => {});
    } else {
      console.info("[codebar]", message);
    }
  },
  /** 在资源管理器中打开数据目录(含日志文件) */
  openLogDir(): Promise<void> {
    if (isTauri) return invoke<void>("open_log_dir");
    console.info("[codebar] openLogDir(mock)");
    return Promise.resolve();
  },
  /** 订阅刷新结果事件;返回取消函数 */
  onUsageUpdated(cb: (payload: UsageUpdatedPayload) => void): () => void {
    if (!isTauri) return mock.onUsage(cb as (p: unknown) => void);
    let unlisten: (() => void) | null = null;
    let disposed = false;
    import("@tauri-apps/api/event").then(({ listen }) =>
      listen<UsageUpdatedPayload>("usage://updated", (e) => cb(e.payload)).then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
    );
    return () => {
      disposed = true;
      unlisten?.();
    };
  },
  /** 订阅配置变更事件(跨窗口主题同步等) */
  onConfigUpdated(cb: (payload: ConfigUpdatedPayload) => void): () => void {
    if (!isTauri) return mock.onConfig(cb as (p: unknown) => void);
    let unlisten: (() => void) | null = null;
    let disposed = false;
    import("@tauri-apps/api/event").then(({ listen }) =>
      listen<ConfigUpdatedPayload>("config://updated", (e) => cb(e.payload)).then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
    );
    return () => {
      disposed = true;
      unlisten?.();
    };
  },
};
