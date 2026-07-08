import { invoke } from "@tauri-apps/api/core";
import "./styles.css";

type Tab = "overview" | "mouse" | "network" | "update" | "settings";
type AppMode = "idle" | "host" | "remote";
type KeyboardTarget = "local" | "remote";
type Theme = "light" | "soft";
type MouseSensitivity = "stable" | "balanced" | "sensitive";
type StartupMode = "last" | "host" | "remote" | "idle";

type AppStatus = {
  version: string;
  mode: AppMode;
  running: boolean;
  connected: boolean;
  target: KeyboardTarget;
  elevated: boolean;
  logs: string[];
  config: {
    tcpPort: number;
    discoveryPort: number;
    updatePort: number;
    remoteHost: string | null;
    startupMode: StartupMode;
    lastMode: string;
    restoreLastMode: boolean;
    startOnLogin: boolean;
    minimizeToTray: boolean;
    autoDiscovery: boolean;
    gameMode: boolean;
    theme: Theme;
    mouseSensitivity: MouseSensitivity;
    mouseFollow: {
      enabled: boolean;
      hostMouseReturnsLocal: boolean;
      remoteMouseSwitchesRemote: boolean;
      hostPollIntervalMs: number;
      remoteReportIntervalMs: number;
      hostPriorityCooldownMs: number;
      switchDebounceMs: number;
    };
  };
};

type NetworkDiagnostics = {
  localIps: string[];
  tcpPort: number;
  discoveryPort: number;
  updatePort: number;
  configuredHost: string | null;
  autoDiscovery: boolean;
  runningMode: AppMode;
  connected: boolean;
  keyboardTarget: KeyboardTarget;
  targetHost: string | null;
  tcpReachable: boolean | null;
  updateReachable: boolean | null;
};

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) throw new Error("Missing app root");
const appRoot = app;

let activeTab: Tab = "overview";
let autoFollowLogs = true;
let pendingAction: string | null = null;
let diagnostics: NetworkDiagnostics | null = null;

let status: AppStatus = {
  version: "0.1.13",
  mode: "idle",
  running: false,
  connected: false,
  target: "local",
  elevated: false,
  logs: [],
  config: {
    tcpPort: 8765,
    discoveryPort: 8766,
    updatePort: 8767,
    remoteHost: null,
    startupMode: "last",
    lastMode: "idle",
    restoreLastMode: true,
    startOnLogin: false,
    minimizeToTray: false,
    autoDiscovery: true,
    gameMode: false,
    theme: "light",
    mouseSensitivity: "balanced",
    mouseFollow: {
      enabled: true,
      hostMouseReturnsLocal: true,
      remoteMouseSwitchesRemote: true,
      hostPollIntervalMs: 20,
      remoteReportIntervalMs: 40,
      hostPriorityCooldownMs: 60,
      switchDebounceMs: 80
    }
  }
};

async function refreshStatus() {
  status = await invoke<AppStatus>("app_status");
  render();
}

async function runAction(name: string, action: () => Promise<unknown>) {
  pendingAction = name;
  render();
  try {
    await action();
    await refreshStatus();
  } finally {
    pendingAction = null;
    render();
  }
}

async function startMode(mode: "host" | "remote") {
  await runAction(`start-${mode}`, () => invoke("start_mode", { mode }));
}

async function stopMode() {
  await runAction("stop", () => invoke("stop_mode"));
}

async function saveRemoteHost() {
  const input = document.querySelector<HTMLInputElement>("#remote-host");
  await runAction("save-host", () => invoke("set_remote_host", { host: input?.value || null }));
  await refreshDiagnostics();
}

async function setKeyboardTarget(target: KeyboardTarget) {
  await runAction(`target-${target}`, () => invoke("set_keyboard_target", { target }));
}

async function restartAsAdmin() {
  await runAction("restart-admin", () => invoke("restart_as_admin"));
}

async function setTheme(theme: Theme) {
  await runAction(`theme-${theme}`, () => invoke("set_theme", { theme }));
}

async function setStartOnLogin(enabled: boolean) {
  await runAction("start-login", () => invoke("set_start_on_login", { enabled }));
}

async function setRestoreLastMode(enabled: boolean) {
  await runAction("restore-last-mode", () => invoke("set_restore_last_mode", { enabled }));
}

async function setStartupMode(mode: StartupMode) {
  await runAction(`startup-${mode}`, () => invoke("set_startup_mode", { mode }));
}

async function setMinimizeOnStart(enabled: boolean) {
  await runAction("minimize-on-start", () => invoke("set_minimize_to_tray", { enabled }));
}

async function setAutoDiscovery(enabled: boolean) {
  await runAction("auto-discovery", () => invoke("set_auto_discovery", { enabled }));
  await refreshDiagnostics();
}

async function setGameMode(enabled: boolean) {
  await runAction("game-mode", () => invoke("set_game_mode", { enabled }));
}

async function setMouseSensitivity(preset: MouseSensitivity) {
  await runAction(`mouse-${preset}`, () => invoke("set_mouse_sensitivity", { preset }));
}

async function refreshDiagnostics() {
  diagnostics = await invoke<NetworkDiagnostics>("network_diagnostics");
  render();
}

async function copyLogs() {
  const payload = compactLogs(status.logs);
  try {
    await navigator.clipboard.writeText(payload);
  } catch {
    const log = document.querySelector<HTMLTextAreaElement>("#log-text");
    log?.select();
    document.execCommand("copy");
  }
}

async function clearLogs() {
  await runAction("clear-logs", () => invoke("clear_logs"));
}

function downloadLogs() {
  const blob = new Blob([status.logs.join("")], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = `devices-router-log-${new Date().toISOString().replace(/[:.]/g, "-")}.txt`;
  link.click();
  URL.revokeObjectURL(url);
}

function render() {
  const oldLog = document.querySelector<HTMLTextAreaElement>("#log-text");
  const oldScrollTop = oldLog?.scrollTop ?? 0;
  const wasAtBottom = oldLog ? oldLog.scrollTop + oldLog.clientHeight >= oldLog.scrollHeight - 24 : true;

  appRoot.innerHTML = `
    <main class="shell theme-${status.config.theme}">
      <aside class="sidebar">
        <div class="brand">Devices Router</div>
        ${navButton("overview", "总览")}
        ${navButton("mouse", "鼠标跟随")}
        ${navButton("network", "网络诊断")}
        ${navButton("update", "更新")}
        ${navButton("settings", "设置")}
      </aside>
      <section class="content">
        <header class="topbar">
          <div>
            <h1>键盘跟随控制台</h1>
            <p>版本 v${status.version}</p>
          </div>
          <div class="badge ${status.running ? "ok" : ""}">${status.running ? "运行中" : "未启动"}</div>
        </header>
        ${renderTab()}
        ${renderLogs()}
      </section>
    </main>
  `;

  bindEvents();
  const newLog = document.querySelector<HTMLTextAreaElement>("#log-text");
  if (newLog) {
    newLog.scrollTop = autoFollowLogs && wasAtBottom ? newLog.scrollHeight : oldScrollTop;
  }
}

function navButton(tab: Tab, label: string) {
  return `<button class="nav ${activeTab === tab ? "active" : ""}" data-tab="${tab}">${label}</button>`;
}

function renderTab() {
  if (activeTab === "mouse") return renderMouseTab();
  if (activeTab === "network") return renderNetworkTab();
  if (activeTab === "update") return renderUpdateTab();
  if (activeTab === "settings") return renderSettingsTab();
  return renderOverviewTab();
}

function renderOverviewTab() {
  return `
    <section class="workspace">
      <article class="panel">
        <h2>运行模式</h2>
        <div class="actions">
          ${actionButton("start-host", "主电脑模式", status.mode === "host")}
          ${actionButton("start-remote", "副电脑模式", status.mode === "remote")}
          ${actionButton("stop", "停止", status.mode === "idle")}
        </div>
      </article>
      <article class="panel">
        <h2>当前状态</h2>
        ${definitionList([
          ["模式", modeLabel(status.mode)],
          ["连接", status.connected ? "已连接" : "未连接"],
          ["键盘目标", targetLabel(status.target)],
          ["管理员权限", status.elevated ? "已开启" : "未开启"]
        ])}
        ${renderElevationHint()}
      </article>
      <article class="panel wide">
        <h2>键盘切换</h2>
        <div class="actions">
          ${actionButton("target-local", "键盘到主电脑", status.target === "local")}
          ${actionButton("target-remote", "键盘到副电脑", status.target === "remote")}
        </div>
      </article>
    </section>
  `;
}

function renderMouseTab() {
  const preset = status.config.mouseSensitivity;
  return `
    <section class="workspace">
      <article class="panel">
        <h2>鼠标跟随</h2>
        ${definitionList([
          ["自动跟随", onOff(status.config.mouseFollow.enabled)],
          ["主电脑移动切回", onOff(status.config.mouseFollow.hostMouseReturnsLocal)],
          ["副电脑移动切过去", onOff(status.config.mouseFollow.remoteMouseSwitchesRemote)],
          ["游戏模式", onOff(status.config.gameMode)]
        ])}
      </article>
      <article class="panel">
        <h2>灵敏度</h2>
        <div class="actions">
          ${actionButton("mouse-stable", "稳定", preset === "stable")}
          ${actionButton("mouse-balanced", "平衡", preset === "balanced")}
          ${actionButton("mouse-sensitive", "灵敏", preset === "sensitive")}
        </div>
        ${definitionList([
          ["主电脑轮询", `${status.config.mouseFollow.hostPollIntervalMs}ms`],
          ["副电脑上报", `${status.config.mouseFollow.remoteReportIntervalMs}ms`],
          ["主电脑优先冷却", `${status.config.mouseFollow.hostPriorityCooldownMs}ms`],
          ["切换防抖", `${status.config.mouseFollow.switchDebounceMs}ms`]
        ])}
      </article>
    </section>
  `;
}

function renderNetworkTab() {
  const info = diagnostics;
  const tcpAdvice = portProbeAdvice(info?.tcpReachable, info?.targetHost, "keyboard");
  const updateAdvice = portProbeAdvice(info?.updateReachable, info?.targetHost, "update");
  return `
    <section class="workspace">
      <article class="panel wide">
        <h2>主电脑地址</h2>
        <div class="inline-form">
          <input id="remote-host" value="${escapeHtml(status.config.remoteHost || "")}" placeholder="自动发现，或填写主电脑 IP" />
          ${actionButton("save-host", "保存", false)}
        </div>
        ${definitionList([
          ["主电脑地址", escapeHtml(status.config.remoteHost || "自动发现")],
          ["自动发现", onOff(status.config.autoDiscovery)],
          ["键盘端口", String(status.config.tcpPort)],
          ["发现端口", String(status.config.discoveryPort)],
          ["更新端口", String(status.config.updatePort)]
        ])}
      </article>
      <article class="panel wide">
        <div class="panel-title-row">
          <h2>网络诊断</h2>
          ${actionButton("refresh-diagnostics", "刷新", false)}
        </div>
        ${definitionList([
          ["本机 LAN IP", escapeHtml(info?.localIps.join(", ") || "点击刷新获取")],
          ["检测目标", escapeHtml(info?.targetHost || "未填写，当前依赖自动发现")],
          ["键盘端口检测", portProbeLabel(info?.tcpReachable)],
          ["更新端口检测", portProbeLabel(info?.updateReachable)],
          ["处理建议", escapeHtml([tcpAdvice, updateAdvice].filter(Boolean).join("；") || "端口可连接时，网络链路基本正常。")],
          ["运行模式", info ? modeLabel(info.runningMode) : "-"],
          ["连接状态", info?.connected ? "已连接" : "未连接"],
          ["键盘目标", info ? targetLabel(info.keyboardTarget) : "-"]
        ])}
      </article>
    </section>
  `;
}

function renderUpdateTab() {
  return `
    <section class="workspace">
      <article class="panel wide">
        <h2>更新</h2>
        ${definitionList([
          ["当前版本", `v${status.version}`],
          ["主电脑更新服务", `${status.config.updatePort} 端口`],
          ["副电脑自动更新", "连上主电脑后自动检查"],
          ["更新包目录", "updates/manifest.json"]
        ])}
      </article>
    </section>
  `;
}

function renderSettingsTab() {
  return `
    <section class="workspace">
      <article class="panel">
        <h2>启动偏好</h2>
        <div class="settings-block">
          <span>启动默认模式</span>
          <div class="actions">
            ${actionButton("startup-last", "沿用上次", status.config.startupMode === "last")}
            ${actionButton("startup-host", "主电脑", status.config.startupMode === "host")}
            ${actionButton("startup-remote", "副电脑", status.config.startupMode === "remote")}
            ${actionButton("startup-idle", "不自动启动", status.config.startupMode === "idle")}
          </div>
        </div>
        ${toggleRow("开机自动启动", "start-login", status.config.startOnLogin)}
        ${toggleRow("启动后最小化", "minimize-on-start", status.config.minimizeToTray)}
        ${definitionList([
          ["上次模式", modeLabel((status.config.lastMode as AppMode) || "idle")],
          ["兼容开关", status.config.restoreLastMode ? "沿用上次模式" : "固定启动模式"]
        ])}
      </article>
      <article class="panel">
        <h2>安全和发现</h2>
        ${toggleRow("自动寻找主电脑", "auto-discovery", status.config.autoDiscovery)}
        ${toggleRow("游戏模式", "game-mode", status.config.gameMode)}
      </article>
      <article class="panel wide">
        <h2>界面主题</h2>
        <div class="actions">
          ${actionButton("theme-light", "清爽浅色", status.config.theme === "light")}
          ${actionButton("theme-soft", "柔和浅色", status.config.theme === "soft")}
        </div>
      </article>
    </section>
  `;
}

function renderLogs() {
  return `
    <section class="panel log-panel">
      <div class="panel-title-row">
        <h2>日志</h2>
        <div class="mini-actions">
          ${actionButton("toggle-autolog", autoFollowLogs ? "暂停跟随" : "继续跟随", !autoFollowLogs)}
          ${actionButton("clear-logs", "清空", false)}
          ${actionButton("copy-logs", "复制", false)}
          ${actionButton("download-logs", "导出", false)}
        </div>
      </div>
      <p class="hint">复制会复制上面折叠后的日志；导出会保存原始完整日志，适合排查问题。</p>
      <textarea id="log-text" readonly spellcheck="false">${escapeHtml(compactLogs(status.logs) || "等待启动...")}</textarea>
    </section>
  `;
}

function bindEvents() {
  document.querySelectorAll<HTMLButtonElement>("[data-tab]").forEach((button) => {
    button.addEventListener("click", () => {
      activeTab = button.dataset.tab as Tab;
      if (activeTab === "network") refreshDiagnostics();
      render();
    });
  });
  onClick("start-host", () => startMode("host"));
  onClick("start-remote", () => startMode("remote"));
  onClick("stop", stopMode);
  onClick("save-host", saveRemoteHost);
  onClick("target-local", () => setKeyboardTarget("local"));
  onClick("target-remote", () => setKeyboardTarget("remote"));
  onClick("restart-admin", restartAsAdmin);
  onClick("start-login", () => setStartOnLogin(!status.config.startOnLogin));
  onClick("restore-last-mode", () => setRestoreLastMode(!status.config.restoreLastMode));
  onClick("startup-last", () => setStartupMode("last"));
  onClick("startup-host", () => setStartupMode("host"));
  onClick("startup-remote", () => setStartupMode("remote"));
  onClick("startup-idle", () => setStartupMode("idle"));
  onClick("minimize-on-start", () => setMinimizeOnStart(!status.config.minimizeToTray));
  onClick("auto-discovery", () => setAutoDiscovery(!status.config.autoDiscovery));
  onClick("game-mode", () => setGameMode(!status.config.gameMode));
  onClick("theme-light", () => setTheme("light"));
  onClick("theme-soft", () => setTheme("soft"));
  onClick("mouse-stable", () => setMouseSensitivity("stable"));
  onClick("mouse-balanced", () => setMouseSensitivity("balanced"));
  onClick("mouse-sensitive", () => setMouseSensitivity("sensitive"));
  onClick("refresh-diagnostics", refreshDiagnostics);
  onClick("copy-logs", copyLogs);
  onClick("clear-logs", clearLogs);
  onClick("download-logs", downloadLogs);
  onClick("toggle-autolog", () => {
    autoFollowLogs = !autoFollowLogs;
    render();
  });
  document.querySelector("#log-text")?.addEventListener("scroll", (event) => {
    const log = event.currentTarget as HTMLTextAreaElement;
    autoFollowLogs = log.scrollTop + log.clientHeight >= log.scrollHeight - 24;
  });
}

function onClick(id: string, handler: () => void | Promise<void>) {
  document.querySelector(`#${id}`)?.addEventListener("click", () => {
    void handler();
  });
}

function actionButton(id: string, label: string, selected: boolean) {
  const busy = pendingAction === id;
  return `<button id="${id}" class="${selected ? "selected" : ""}" ${busy ? "disabled" : ""}>${busy ? "处理中..." : label}</button>`;
}

function toggleRow(label: string, id: string, enabled: boolean) {
  return `
    <div class="settings-row">
      <span>${label}</span>
      ${actionButton(id, enabled ? "已开启" : "已关闭", enabled)}
    </div>
  `;
}

function definitionList(items: Array<[string, string]>) {
  return `
    <dl>
      ${items.map(([key, value]) => `<div><dt>${key}</dt><dd>${value}</dd></div>`).join("")}
    </dl>
  `;
}

function modeLabel(mode: AppMode) {
  if (mode === "host") return "主电脑";
  if (mode === "remote") return "副电脑";
  return "空闲";
}

function targetLabel(target: KeyboardTarget) {
  return target === "remote" ? "副电脑" : "主电脑";
}

function renderElevationHint() {
  if (status.elevated) return "";
  return `
    <div class="notice">
      <strong>控制管理员窗口需要管理员权限</strong>
      <p>如果副电脑上的 PowerShell、Terminal 或 IDE 是管理员身份运行，Devices Router 也需要管理员身份运行。</p>
      ${actionButton("restart-admin", "以管理员身份重启", false)}
    </div>
  `;
}

function onOff(enabled: boolean) {
  return enabled ? "开启" : "关闭";
}

function portProbeLabel(value: boolean | null | undefined) {
  if (value === true) return "可连接";
  if (value === false) return "不可连接";
  return "未检测";
}

function portProbeAdvice(value: boolean | null | undefined, targetHost: string | null | undefined, kind: "keyboard" | "update") {
  if (value !== false) return "";
  if (!targetHost) return "先填写主电脑 IP，或开启自动寻找主电脑";
  const service = kind === "keyboard" ? "主电脑模式" : "主电脑更新服务";
  return `${kind === "keyboard" ? "键盘端口" : "更新端口"}不可连接：确认 ${targetHost} 上已启动 ${service}，两台电脑在同一局域网，Windows 防火墙允许 Devices Router`;
}

function compactLogs(logs: string[]) {
  const lines = logs.join("").split(/\r?\n/);
  const compacted: string[] = [];
  let previous = "";
  let repeated = 0;

  for (const line of lines) {
    if (!line) continue;
    if (line === previous) {
      repeated += 1;
      continue;
    }
    flushRepeated();
    compacted.push(line);
    previous = line;
    repeated = 0;
  }
  flushRepeated();
  return compacted.join("\n");

  function flushRepeated() {
    if (repeated > 0) {
      compacted.push(`  ↳ 上一条重复 ${repeated} 次`);
    }
  }
}

function escapeHtml(value: string) {
  return value.replace(/[&<>"']/g, (char) => {
    const map: Record<string, string> = {
      "&": "&amp;",
      "<": "&lt;",
      ">": "&gt;",
      "\"": "&quot;",
      "'": "&#039;"
    };
    return map[char];
  });
}

render();
refreshStatus();
setInterval(refreshStatus, 200);
