import { invoke } from "@tauri-apps/api/core";
import "./styles.css";

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

let status: AppStatus = {
  version: "0.1.2",
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
  appRoot.innerHTML = `
    <main class="shell">
      <aside class="sidebar">
        <div class="brand">Devices Router</div>
        <button class="nav active">总览</button>
        <button class="nav">鼠标跟随</button>
        <button class="nav">网络诊断</button>
        <button class="nav">更新</button>
      </aside>
      <section class="content">
        <header class="topbar">
          <div>
            <h1>键盘跟随控制台</h1>
            <p>版本 v${status.version}</p>
          </div>
          <div class="badge ${status.running ? "ok" : ""}">${status.running ? "运行中" : "未启动"}</div>
        </header>
        <section class="grid">
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
              <div><dt>模式</dt><dd>${status.mode}</dd></div>
              <div><dt>连接</dt><dd>${status.connected ? "已连接" : "未连接"}</dd></div>
              <div><dt>键盘目标</dt><dd>${status.target === "remote" ? "副电脑" : "主电脑"}</dd></div>
            </dl>
          </article>
        </section>
        <section class="grid">
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
            </dl>
          </article>
        </section>
        <section class="panel">
          <h2>网络</h2>
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
        </section>
        <section class="panel log-panel">
          <div class="panel-title-row">
            <h2>日志</h2>
            <div class="mini-actions">
              <button id="copy-logs">复制日志</button>
              <button id="download-logs">导出日志</button>
            </div>
          </div>
          <textarea id="log-text" readonly spellcheck="false">${escapeHtml(status.logs.join("") || "等待启动...")}</textarea>
        </section>
      </section>
    </main>
  `;
  document.querySelector("#start-host")?.addEventListener("click", () => startMode("host"));
  document.querySelector("#start-remote")?.addEventListener("click", () => startMode("remote"));
  document.querySelector("#stop")?.addEventListener("click", stopMode);
  document.querySelector("#save-host")?.addEventListener("click", saveRemoteHost);
  document.querySelector("#copy-logs")?.addEventListener("click", copyLogs);
  document.querySelector("#download-logs")?.addEventListener("click", downloadLogs);
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
