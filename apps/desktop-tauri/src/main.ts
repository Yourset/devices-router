import { invoke } from "@tauri-apps/api/core";
import "./styles.css";

type Tab = "overview" | "mouse" | "network" | "update";

type AppStatus = {
  version: string;
  mode: "idle" | "host" | "remote";
  running: boolean;
  connected: boolean;
  target: "local" | "remote";
  logs: string[];
  config: {
    tcpPort: number;
    discoveryPort: number;
    updatePort: number;
    remoteHost: string | null;
    lastMode: string;
    startOnLogin: boolean;
    theme: "light" | "soft";
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

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) throw new Error("Missing app root");
const appRoot = app;

let activeTab: Tab = "overview";
let autoFollowLogs = true;
let status: AppStatus = {
  version: "0.1.7",
  mode: "idle",
  running: false,
  connected: false,
  target: "local",
  logs: [],
  config: {
    tcpPort: 8765,
    discoveryPort: 8766,
    updatePort: 8767,
    remoteHost: null,
    lastMode: "idle",
    startOnLogin: false,
    theme: "light",
    mouseFollow: {
      enabled: true,
      hostMouseReturnsLocal: true,
      remoteMouseSwitchesRemote: true,
      hostPollIntervalMs: 50,
      remoteReportIntervalMs: 500,
      hostPriorityCooldownMs: 800,
      switchDebounceMs: 300
    }
  }
};

async function refreshStatus() {
  status = await invoke<AppStatus>("app_status");
  render();
}

async function startMode(mode: "host" | "remote") {
  await invoke("start_mode", { mode });
  await refreshStatus();
}

async function stopMode() {
  await invoke("stop_mode");
  await refreshStatus();
}

async function saveRemoteHost() {
  const input = document.querySelector<HTMLInputElement>("#remote-host");
  await invoke("set_remote_host", { host: input?.value || null });
  await refreshStatus();
}

async function setKeyboardTarget(target: "local" | "remote") {
  await invoke("set_keyboard_target", { target });
  await refreshStatus();
}

async function setTheme(theme: "light" | "soft") {
  await invoke("set_theme", { theme });
  await refreshStatus();
}

async function setStartOnLogin(enabled: boolean) {
  await invoke("set_start_on_login", { enabled });
  await refreshStatus();
}

async function copyLogs() {
  await navigator.clipboard.writeText(status.logs.join(""));
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
    if (autoFollowLogs && wasAtBottom) {
      newLog.scrollTop = newLog.scrollHeight;
    } else {
      newLog.scrollTop = oldScrollTop;
    }
  }
}

function navButton(tab: Tab, label: string) {
  return `<button class="nav ${activeTab === tab ? "active" : ""}" data-tab="${tab}">${label}</button>`;
}

function renderTab() {
  if (activeTab === "mouse") return renderMouseTab();
  if (activeTab === "network") return renderNetworkTab();
  if (activeTab === "update") return renderUpdateTab();
  return renderOverviewTab();
}

function renderOverviewTab() {
  return `
    <section class="workspace">
      <article class="panel">
        <h2>运行模式</h2>
        <div class="actions">
          <button id="start-host">主电脑模式</button>
          <button id="start-remote">副电脑模式</button>
          <button id="stop">停止</button>
        </div>
      </article>
      <article class="panel">
        <h2>当前状态</h2>
        <dl>
          <div><dt>模式</dt><dd>${modeLabel(status.mode)}</dd></div>
          <div><dt>连接</dt><dd>${status.connected ? "已连接" : "未连接"}</dd></div>
          <div><dt>键盘目标</dt><dd>${status.target === "remote" ? "副电脑" : "主电脑"}</dd></div>
        </dl>
      </article>
      <article class="panel wide">
        <h2>键盘切换</h2>
        <div class="actions">
          <button id="target-local">键盘到主电脑</button>
          <button id="target-remote">键盘到副电脑</button>
        </div>
      </article>
    </section>
  `;
}

function renderMouseTab() {
  return `
    <section class="workspace">
      <article class="panel">
        <h2>鼠标跟随</h2>
        <dl>
          <div><dt>自动跟随</dt><dd>${status.config.mouseFollow.enabled ? "开启" : "关闭"}</dd></div>
          <div><dt>主电脑移动切回</dt><dd>${status.config.mouseFollow.hostMouseReturnsLocal ? "开启" : "关闭"}</dd></div>
          <div><dt>副电脑移动切过去</dt><dd>${status.config.mouseFollow.remoteMouseSwitchesRemote ? "开启" : "关闭"}</dd></div>
        </dl>
      </article>
      <article class="panel">
        <h2>频率配置</h2>
        <dl>
          <div><dt>主电脑轮询</dt><dd>${status.config.mouseFollow.hostPollIntervalMs}ms</dd></div>
          <div><dt>副电脑上报</dt><dd>${status.config.mouseFollow.remoteReportIntervalMs}ms</dd></div>
          <div><dt>主电脑优先冷却</dt><dd>${status.config.mouseFollow.hostPriorityCooldownMs}ms</dd></div>
          <div><dt>切换防抖</dt><dd>${status.config.mouseFollow.switchDebounceMs}ms</dd></div>
        </dl>
      </article>
    </section>
  `;
}

function renderNetworkTab() {
  return `
    <section class="workspace">
      <article class="panel wide">
        <h2>主电脑地址</h2>
        <div class="inline-form">
          <input id="remote-host" value="${escapeHtml(status.config.remoteHost || "")}" placeholder="自动发现，或填写主电脑 IP" />
          <button id="save-host">保存</button>
        </div>
        <dl>
          <div><dt>主电脑地址</dt><dd>${escapeHtml(status.config.remoteHost || "自动发现")}</dd></div>
          <div><dt>键盘端口</dt><dd>${status.config.tcpPort}</dd></div>
          <div><dt>发现端口</dt><dd>${status.config.discoveryPort}</dd></div>
          <div><dt>更新端口</dt><dd>${status.config.updatePort}</dd></div>
        </dl>
      </article>
    </section>
  `;
}

function renderUpdateTab() {
  return `
    <section class="workspace">
      <article class="panel wide">
        <h2>更新</h2>
        <dl>
          <div><dt>当前版本</dt><dd>v${status.version}</dd></div>
          <div><dt>主电脑更新服务</dt><dd>${status.config.updatePort} 端口</dd></div>
          <div><dt>副电脑自动更新</dt><dd>连上主电脑后自动检查</dd></div>
          <div><dt>更新包目录</dt><dd>updates/manifest.json</dd></div>
        </dl>
      </article>
      <article class="panel wide">
        <h2>偏好</h2>
        <div class="settings-row">
          <span>开机自动启动</span>
          <button id="toggle-start-login">${status.config.startOnLogin ? "已开启" : "已关闭"}</button>
        </div>
        <div class="settings-row">
          <span>界面主题</span>
          <div class="segmented">
            <button id="theme-light" class="${status.config.theme === "light" ? "selected" : ""}">清爽浅色</button>
            <button id="theme-soft" class="${status.config.theme === "soft" ? "selected" : ""}">柔和浅色</button>
          </div>
        </div>
        <dl>
          <div><dt>上次模式</dt><dd>${modeLabel((status.config.lastMode as AppStatus["mode"]) || "idle")}</dd></div>
        </dl>
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
          <button id="toggle-autolog">${autoFollowLogs ? "停止跟随" : "跟随最新"}</button>
          <button id="copy-logs">复制日志</button>
          <button id="download-logs">导出日志</button>
        </div>
      </div>
      <textarea id="log-text" readonly spellcheck="false">${escapeHtml(status.logs.join("") || "等待启动...")}</textarea>
    </section>
  `;
}

function bindEvents() {
  document.querySelectorAll<HTMLButtonElement>("[data-tab]").forEach((button) => {
    button.addEventListener("click", () => {
      activeTab = button.dataset.tab as Tab;
      render();
    });
  });
  document.querySelector("#start-host")?.addEventListener("click", () => startMode("host"));
  document.querySelector("#start-remote")?.addEventListener("click", () => startMode("remote"));
  document.querySelector("#stop")?.addEventListener("click", stopMode);
  document.querySelector("#save-host")?.addEventListener("click", saveRemoteHost);
  document.querySelector("#target-local")?.addEventListener("click", () => setKeyboardTarget("local"));
  document.querySelector("#target-remote")?.addEventListener("click", () => setKeyboardTarget("remote"));
  document.querySelector("#toggle-start-login")?.addEventListener("click", () => setStartOnLogin(!status.config.startOnLogin));
  document.querySelector("#theme-light")?.addEventListener("click", () => setTheme("light"));
  document.querySelector("#theme-soft")?.addEventListener("click", () => setTheme("soft"));
  document.querySelector("#copy-logs")?.addEventListener("click", copyLogs);
  document.querySelector("#download-logs")?.addEventListener("click", downloadLogs);
  document.querySelector("#toggle-autolog")?.addEventListener("click", () => {
    autoFollowLogs = !autoFollowLogs;
    render();
  });
  document.querySelector("#log-text")?.addEventListener("scroll", (event) => {
    const log = event.currentTarget as HTMLTextAreaElement;
    autoFollowLogs = log.scrollTop + log.clientHeight >= log.scrollHeight - 24;
  });
}

function modeLabel(mode: AppStatus["mode"]) {
  if (mode === "host") return "主电脑";
  if (mode === "remote") return "副电脑";
  return "空闲";
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
setInterval(refreshStatus, 1000);
