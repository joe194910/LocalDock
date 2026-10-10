import {
  AppWindow, Boxes, Check, CheckCircle2, ClipboardList, Clock3, Code2, Copy,
  Database, ExternalLink, FileText, FolderOpen, Gauge, Home, Info, Link2, LoaderCircle,
  LockKeyhole, Minus, PackagePlus, Play, Plus, RefreshCw, Search, Settings,
  Square, Store, Wrench, X,
} from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useMemo, useState } from "react";
import {
  cancelTask, getAppSettings, getAppVersion, getJenkinsInitialPassword, getStorageLocations, isTauriRuntime, loadWorkspace,
  openServiceEndpoint, openStorageDirectory, probeConnection, saveExternalConnection,
  scanLocalMachine, setDefaultDeploymentBackend, startPackageInstall, testConnection,
} from "./api";
import type {
  AppSettings, CatalogPackage, ConnectionDraft, ConnectionProbeResult, DeploymentTask,
  ManagementKind, ServiceRecord, StorageLocations, ToolRecord, WorkspaceSnapshot,
} from "./types";

type PageKey = "overview" | "tools" | "services" | "catalog" | "tasks" | "logs" | "settings";
type ServiceFilter = "all" | "localdock_compose" | "1panel_compose" | "external_container" | "external_connection";

const emptyWorkspace: WorkspaceSnapshot = {
  mode: "unavailable",
  scannedAt: "",
  services: [],
  tools: [],
  tasks: [],
  panel: { state: "not_found", message: "尚未连接 LocalDock 桌面后端。" },
  packages: [],
  notices: [],
};

const navItems: Array<{ key: PageKey; label: string; icon: typeof Home }> = [
  { key: "overview", label: "概览", icon: Home },
  { key: "tools", label: "开发环境", icon: AppWindow },
  { key: "services", label: "服务与连接", icon: Database },
  { key: "catalog", label: "应用目录", icon: Store },
  { key: "tasks", label: "部署任务", icon: ClipboardList },
  { key: "logs", label: "诊断日志", icon: FileText },
  { key: "settings", label: "设置", icon: Settings },
];

const sourceText: Record<ManagementKind, string> = {
  localdock_compose: "LocalDock Compose",
  "1panel_compose": "1Panel 编排",
  external_connection: "外部连接",
  external_container: "外部容器",
  localdock_windows_tool: "LocalDock 管理",
  external_windows_tool: "外部安装",
};

const appTone: Record<ServiceRecord["appId"], string> = {
  mysql: "mysql",
  redis: "redis",
  minio: "minio",
  qdrant: "qdrant",
  jenkins: "jenkins",
};

const defaultPorts: Record<ConnectionDraft["appId"], number> = {
  mysql: 3306,
  redis: 6379,
  minio: 9001,
  qdrant: 6333,
  jenkins: 8080,
};

function Logo() {
  return <span className="logo"><Boxes size={18} strokeWidth={2.5} /></span>;
}

function Button({
  children, kind = "secondary", onClick, disabled = false, title, loading = false, loadingLabel = "处理中",
}: {
  children: React.ReactNode;
  kind?: "primary" | "secondary" | "danger";
  onClick?: () => void;
  disabled?: boolean;
  title?: string;
  loading?: boolean;
  loadingLabel?: string;
}) {
  return (
    <button
      className={`button button--${kind} ${loading ? "is-loading" : ""}`}
      onClick={onClick}
      disabled={disabled || loading}
      title={title}
      aria-busy={loading}
    >
      {loading ? <><LoaderCircle className="spin" size={16} /> {loadingLabel}</> : children}
    </button>
  );
}

async function writeClipboardText(value: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(value);
    return;
  }

  const textarea = document.createElement("textarea");
  textarea.value = value;
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.select();
  const copied = document.execCommand("copy");
  textarea.remove();
  if (!copied) throw new Error("系统拒绝访问剪贴板。");
}

function CopyButton({ value, label }: { value: string; label: string }) {
  const [state, setState] = useState<"idle" | "copied" | "failed">("idle");

  const copy = async () => {
    try {
      await writeClipboardText(value);
      setState("copied");
    } catch {
      setState("failed");
    }
    window.setTimeout(() => setState("idle"), 1800);
  };

  return (
    <Button onClick={copy} disabled={!value}>
      {state === "copied"
        ? <><Check size={16} /> 已复制</>
        : state === "failed"
          ? <><X size={16} /> 复制失败</>
          : <><Copy size={16} /> {label}</>}
    </Button>
  );
}

function Badge({ tone, children }: { tone: "green" | "blue" | "orange" | "gray" | "purple" | "red"; children: React.ReactNode }) {
  return <span className={`badge badge--${tone}`}>{children}</span>;
}

function Modal({ title, children, onClose }: { title: string; children: React.ReactNode; onClose: () => void }) {
  return (
    <div className="modal-layer" onMouseDown={onClose}>
      <section className="modal" role="dialog" aria-modal="true" aria-label={title} onMouseDown={(event) => event.stopPropagation()}>
        <header><h2>{title}</h2><button className="icon-button" title="关闭" onClick={onClose}><X size={20} /></button></header>
        {children}
      </section>
    </div>
  );
}

function SummaryCard({ icon: Icon, label, value, tone }: { icon: typeof Database; label: string; value: number; tone: string }) {
  return (
    <div className="summary-card">
      <span className={`summary-card__icon summary-card__icon--${tone}`}><Icon size={27} /></span>
      <div><span>{label}</span><strong>{value}</strong></div>
    </div>
  );
}

function ServiceIcon({ appId }: { appId: ServiceRecord["appId"] }) {
  return <span className={`service-icon service-icon--${appTone[appId]}`}><Database size={21} /></span>;
}

function ConnectionBadge({ service }: { service: ServiceRecord }) {
  if (!service.endpointPublished) return <Badge tone="gray"><Info size={13} /> 未发布主机端口</Badge>;
  if (service.connection === "authenticated") return <Badge tone="green"><CheckCircle2 size={13} /> 协议鉴权通过</Badge>;
  if (service.connection === "reachable") return <Badge tone="orange"><Info size={13} /> 仅端口可达</Badge>;
  if (service.connection === "failed") return <Badge tone="red"><X size={13} /> 端口不可达</Badge>;
  return <Badge tone="gray"><Clock3 size={13} /> 尚未测试</Badge>;
}

function RuntimeBadge({ service }: { service: ServiceRecord }) {
  if (service.runtime === "running") return <Badge tone="green"><Play size={12} fill="currentColor" /> 运行中</Badge>;
  if (service.runtime === "installing") return <Badge tone="blue"><LoaderCircle size={13} /> {service.progressLabel ?? "安装中"}</Badge>;
  if (service.runtime === "paused") return <Badge tone="orange">已暂停</Badge>;
  if (service.runtime === "stopped") return <Badge tone="orange">已停止</Badge>;
  if (service.runtime === "error") return <Badge tone="red">异常</Badge>;
  return <Badge tone="gray">运行状态未知</Badge>;
}

function PageHeader({ title, subtitle, children }: { title: string; subtitle: string; children?: React.ReactNode }) {
  return (
    <div className="page-header">
      <div><h1>{title}</h1><p>{subtitle}</p></div>
      {children && <div className="page-actions">{children}</div>}
    </div>
  );
}

function EmptyState({ title, detail }: { title: string; detail: string }) {
  return <div className="empty"><Database /><strong>{title}</strong><span>{detail}</span></div>;
}

function ServiceDetails({
  service, task, onClose, onTest, onOpen, onCopySetupPassword, busyAction, runtimeReady,
}: {
  service: ServiceRecord;
  task?: DeploymentTask;
  onClose: () => void;
  onTest: () => void;
  onOpen: () => void;
  onCopySetupPassword: () => void;
  busyAction: string;
  runtimeReady: boolean;
}) {
  const canOpen = service.capabilities.includes("open_browser");
  const canTest = service.capabilities.includes("test");
  const canCopy = service.capabilities.includes("copy_connection");
  const passwordText = service.credentialState === "encrypted"
    ? "已加密保存"
    : service.credentialState === "not_required" ? "不需要" : "未保存";
  const logText = task
    ? task.events.map((event) => `${event.time} [${event.level}] ${event.message}`).join("\n")
    : service.lastChecked
      ? `[信息] 最近一次端口探测：${service.lastChecked}\n[信息] 当前结果：${service.connection === "reachable" ? "TCP 端口可达，未执行协议鉴权" : service.connection === "failed" ? "TCP 端口不可达" : service.connection === "authenticated" ? "协议鉴权通过" : "没有可用结果"}`
      : "[信息] 尚未执行真实连接测试";

  return (
    <aside className="details-panel">
      <header><h2>{service.name} 连接详情</h2><button className="icon-button" title="关闭详情" onClick={onClose}><X size={21} /></button></header>
      <div className="details-tags">
        <Badge tone={service.management === "external_connection" ? "gray" : "blue"}><Link2 size={13} /> {sourceText[service.management]}</Badge>
        <ConnectionBadge service={service} />
      </div>
      <section className="detail-card">
        <h3>连接信息</h3>
        <dl>
          <div><dt>主机地址</dt><dd>{service.endpointPublished ? service.host : "未映射到 Windows"}</dd></div>
          <div><dt>端口</dt><dd>{service.endpointPublished ? service.port : `${service.port}（容器端口）`}</dd></div>
          <div><dt>用户名</dt><dd>{service.username ?? "未保存"}</dd></div>
          <div><dt>凭据</dt><dd>{passwordText} <LockKeyhole size={14} /></dd></div>
          <div><dt>TLS</dt><dd>{service.tls === "disabled" ? "未启用" : service.tls === "required" ? "必须验证" : "自定义证书"}</dd></div>
          <div><dt>来源</dt><dd>{sourceText[service.management]}</dd></div>
        </dl>
      </section>
      <section className="detail-card">
        <h3>服务能力</h3>
        {service.endpointPublished
          ? <p className="capability capability--yes"><CheckCircle2 size={18} /> 已开放：端口测试、复制连接信息{canOpen ? "、打开网页入口" : ""}</p>
          : <p className="capability"><Info size={18} /> 容器仍然存在，但没有发布到 Windows 的主机端口。</p>}
        {!service.capabilities.includes("start") && <p className="capability"><Info size={18} /> 启停管理：未授权或不适用</p>}
      </section>
      <div className="details-actions">
        {canTest && <Button
            kind="primary"
            onClick={onTest}
            disabled={!runtimeReady}
            loading={busyAction === `test:${service.id}`}
            loadingLabel="正在探测"
          ><Play size={16} /> 测试端口可达性</Button>}
        {canOpen
          ? <Button onClick={onOpen} disabled={!runtimeReady} loading={busyAction === `open:${service.id}`} loadingLabel="正在打开"><ExternalLink size={16} /> 打开网页入口</Button>
          : canCopy ? <CopyButton value={`${service.host}:${service.port}`} label="复制地址" /> : null}
        {service.appId === "jenkins" && service.management === "localdock_compose" && service.runtime === "running" &&
          <Button
            onClick={onCopySetupPassword}
            disabled={!runtimeReady}
            loading={busyAction === `jenkins-password:${service.id}`}
            loadingLabel="正在读取"
          ><Copy size={16} /> 复制初始密码</Button>}
      </div>
      <section className="detail-card detail-card--log">
        <h3>{task ? "任务真实日志" : "连接测试记录"}</h3>
        <pre>{logText}</pre>
      </section>
      <p className="secure-note"><LockKeyhole size={15} /> 当前记录不会展示或导出明文秘密。</p>
    </aside>
  );
}

function ServicesPage({
  snapshot, onScan, onSelect, selected, busyAction, onTest, onOpen, onAdd, onInstall,
  onCancel, onCopySetupPassword, runtimeReady,
}: {
  snapshot: WorkspaceSnapshot;
  onScan: () => void;
  onSelect: (service: ServiceRecord) => void;
  selected?: ServiceRecord;
  busyAction: string;
  onTest: (service: ServiceRecord) => void;
  onOpen: (service: ServiceRecord) => void;
  onAdd: () => void;
  onInstall: () => void;
  onCancel: (taskId: string) => void;
  onCopySetupPassword: (service: ServiceRecord) => void;
  runtimeReady: boolean;
}) {
  const [filter, setFilter] = useState<ServiceFilter>("all");
  const filtered = snapshot.services.filter((service) => filter === "all" || service.management === filter);
  const authenticated = snapshot.services.filter((service) => service.connection === "authenticated").length;
  const runningTasks = snapshot.tasks.filter((task) => task.status === "running" || task.status === "queued").length;
  const activeTask = snapshot.tasks.find((task) => task.status === "running");
  const filters: Array<[ServiceFilter, string]> = [
    ["all", "全部"],
    ["localdock_compose", "LocalDock 管理"],
    ["1panel_compose", "1Panel 管理"],
    ["external_container", "外部容器"],
    ["external_connection", "外部连接"],
  ];

  return (
    <div className={`page page--services ${selected ? "page--with-details" : ""}`}>
      <main className="service-main">
        <PageHeader title="服务与连接" subtitle="只展示真实保存的资产和真实探测结果。">
          <Button onClick={onScan} disabled={!runtimeReady} loading={busyAction === "scan"} loadingLabel="正在扫描"><Search size={17} /> 扫描本机</Button>
          <Button onClick={onAdd} disabled={!runtimeReady}><Plus size={17} /> 添加已有服务</Button>
          <Button kind="primary" onClick={onInstall}><PackagePlus size={17} /> 查看可用服务包</Button>
        </PageHeader>
        {!runtimeReady && <div className="demo-banner"><Info size={17} /> 当前页面未连接桌面后端，不展示任何模拟业务数据。</div>}
        <div className="summary-grid">
          <SummaryCard icon={Database} label="真实服务记录" value={snapshot.services.length} tone="blue" />
          <SummaryCard icon={CheckCircle2} label="协议鉴权通过" value={authenticated} tone="green" />
          <SummaryCard icon={Clock3} label="真实进行中任务" value={runningTasks} tone="blue" />
        </div>
        <div className="info-strip"><Info size={18} /><strong>TCP 端口可达不等于协议鉴权通过，也不等于服务健康。</strong></div>
        <section className="panel-card">
          <Logo />
          <div className="panel-card__title"><h2>1Panel · 可选增强</h2><p>{snapshot.panel.message}</p></div>
          <Badge tone={snapshot.panel.state === "authorized" ? "green" : snapshot.panel.state === "not_found" ? "gray" : snapshot.panel.state === "error" ? "red" : "blue"}>
            {snapshot.panel.state === "authorized" ? <Check size={13} /> : snapshot.panel.state === "authorization_required" || snapshot.panel.state === "read_only_discovered" ? <Info size={13} /> : <Clock3 size={13} />}
            {snapshot.panel.state === "authorized" ? "API 已接入" : snapshot.panel.state === "not_found" ? "未发现" : snapshot.panel.state === "error" ? "检测异常" : "只读发现"}
          </Badge>
          <span className="panel-note">当前支持真实发现和状态同步，暂不提供 1Panel 写操作。</span>
        </section>
        <div className="filter-tabs">
          {filters.map(([key, label]) => (
            <button key={key} className={filter === key ? "is-active" : ""} onClick={() => setFilter(key)}>
              {label}（{key === "all" ? snapshot.services.length : snapshot.services.filter((item) => item.management === key).length}）
            </button>
          ))}
        </div>
        <section className="service-table">
          <div className="service-row service-row--head">
            <span>服务</span><span>管理来源</span><span>连接状态</span><span>运行状态</span><span>操作</span>
          </div>
          {filtered.length === 0 && <EmptyState title="没有真实服务记录" detail="请在桌面应用中添加已有服务，或等待真实部署后端接入。" />}
          {filtered.map((service) => (
            <div key={service.id} className={`service-row ${selected?.id === service.id ? "is-selected" : ""}`}>
              <div className="service-name"><ServiceIcon appId={service.appId} /><span><strong>{service.name} {service.version}</strong><small>{service.target}</small></span></div>
              <span><Badge tone={service.management === "1panel_compose" ? "purple" : service.management === "external_connection" ? "gray" : "blue"}>{sourceText[service.management]}</Badge></span>
              <span><ConnectionBadge service={service} /></span>
              <span><RuntimeBadge service={service} /></span>
              <span className="row-actions">
                {service.runtime === "installing"
                  ? <Button onClick={() => onSelect(service)}>查看任务</Button>
                  : service.capabilities.includes("open_browser")
                    ? <Button onClick={() => onOpen(service)} disabled={!runtimeReady} loading={busyAction === `open:${service.id}`} loadingLabel="打开中">打开</Button>
                    : service.capabilities.includes("test") ? <Button
                        onClick={() => onTest(service)}
                        disabled={!runtimeReady}
                        loading={busyAction === `test:${service.id}`}
                        loadingLabel="探测中"
                      >测试端口</Button> : <span className="muted-action">未映射端口</span>}
                <Button onClick={() => onSelect(service)}>详情</Button>
              </span>
            </div>
          ))}
        </section>
        {activeTask && (
          <section className="active-task">
            <ServiceIcon appId={snapshot.services.find((item) => item.taskId === activeTask.id)?.appId ?? "qdrant"} />
            <div className="active-task__body">
              <strong>{activeTask.title}</strong><small>{activeTask.stage}</small>
              <div className="progress"><span style={{ width: `${activeTask.progress ?? 0}%` }} /></div>
            </div>
            <strong>{activeTask.progress ?? 0}%</strong>
            <span>{activeTask.transferred ?? "-"} / {activeTask.total ?? "-"}</span>
            <Button onClick={() => {
              const service = snapshot.services.find((item) => item.taskId === activeTask.id);
              if (service) onSelect(service);
            }}><FileText size={16} /> 查看真实日志</Button>
            <Button kind="danger" onClick={() => onCancel(activeTask.id)}><X size={16} /> 取消任务</Button>
          </section>
        )}
        <div className="info-strip info-strip--muted"><Info size={18} /> 添加已有服务只保存连接元数据，不会接管、启动或重启原服务。</div>
      </main>
      {selected && (
        <ServiceDetails
          service={selected}
          task={snapshot.tasks.find((task) => task.id === selected.taskId)}
          onClose={() => onSelect(selected)}
          onTest={() => onTest(selected)}
          onOpen={() => onOpen(selected)}
          onCopySetupPassword={() => onCopySetupPassword(selected)}
          busyAction={busyAction}
          runtimeReady={runtimeReady}
        />
      )}
    </div>
  );
}

function ToolsPage({
  tools, onScan, runtimeReady, busyAction,
}: {
  tools: ToolRecord[];
  onScan: () => void;
  runtimeReady: boolean;
  busyAction: string;
}) {
  return (
    <div className="page">
      <PageHeader title="开发环境" subtitle="检测 Windows 当前 PATH 中实际生效的工具。">
        <Button onClick={onScan} disabled={!runtimeReady} loading={busyAction === "scan"} loadingLabel="正在检测"><RefreshCw size={17} /> 重新检测</Button>
      </PageHeader>
      {!runtimeReady && <div className="demo-banner"><Info size={17} /> 浏览器页面无法读取 Windows 工具，当前列表保持为空。</div>}
      <div className="summary-grid">
        <SummaryCard icon={Wrench} label="实际检测记录" value={tools.length} tone="blue" />
        <SummaryCard icon={Info} label="配置待修复" value={tools.filter((tool) => tool.configuration === "repair_required").length} tone="orange" />
        <SummaryCard icon={Clock3} label="安装任务" value={0} tone="green" />
      </div>
      <section className="generic-table tool-table">
        <div className="generic-row generic-row--head"><span>工具</span><span>已安装版本</span><span>当前生效版本</span><span>配置状态</span><span>管理来源</span><span>操作</span></div>
        {tools.length === 0 && <EmptyState title="尚无真实检测结果" detail="在桌面应用中点击“重新检测”读取本机 PATH。" />}
        {tools.map((tool) => (
          <div className="generic-row" key={tool.id}>
            <strong>{tool.name}</strong>
            <span className="version-cell">{tool.installedVersions.join("、") || "未检测到"}</span>
            <span className="version-cell">{tool.activeVersion ?? "未生效"}</span>
            <span>{tool.configuration === "ready" ? <Badge tone="green">配置正常</Badge> : tool.configuration === "not_installed" ? <Badge tone="gray">未安装</Badge> : <Badge tone="orange">需要修复</Badge>}</span>
            <span>{sourceText[tool.management]}</span>
            <span className="row-actions">{tool.path ? <CopyButton value={tool.path} label="复制路径" /> : <span className="unavailable-note">当前 PATH 未发现可复制路径</span>}</span>
          </div>
        ))}
      </section>
    </div>
  );
}

function CatalogPage({
  packages, services, tools, runtimeReady, busyAction, onView, onInstall,
}: {
  packages: CatalogPackage[];
  services: ServiceRecord[];
  tools: ToolRecord[];
  runtimeReady: boolean;
  busyAction: string;
  onView: (item: CatalogPackage) => void;
  onInstall: (item: CatalogPackage) => void;
}) {
  const [kind, setKind] = useState<"all" | CatalogPackage["kind"]>("all");
  const items = packages.filter((item) => kind === "all" || item.kind === kind);

  const packageState = (item: CatalogPackage) => {
    if (item.kind === "windows_tool") {
      const tool = tools.find((candidate) => candidate.id === item.id);
      if (!tool || tool.configuration === "not_installed") {
        return { tone: "gray" as const, label: "未检测到", detail: "当前 PATH 中未发现该工具。", found: false };
      }
      if (tool.configuration === "repair_required") {
        return { tone: "orange" as const, label: "需要修复", detail: tool.activeVersion ?? "已安装，但当前配置未生效。", found: true };
      }
      return { tone: "green" as const, label: "已安装", detail: tool.activeVersion ?? "已在当前 PATH 中生效。", found: true };
    }

    const matches = services.filter((service) => service.appId === item.id);
    if (matches.length === 0) {
      return { tone: "gray" as const, label: "未发现", detail: "本机扫描结果中没有该服务。", found: false };
    }
    const running = matches.filter((service) => service.runtime === "running");
    const installing = matches.filter((service) => service.runtime === "installing");
    const failed = matches.filter((service) => service.runtime === "error");
    const versions = [...new Set(matches.map((service) => service.version).filter(Boolean))].join("、");
    const versionText = versions ? `，实际版本：${versions}` : "";
    if (running.length > 0) {
      return { tone: "green" as const, label: "运行中", detail: `已发现 ${matches.length} 个实例，其中 ${running.length} 个正在运行${versionText}。`, found: true };
    }
    if (installing.length > 0) {
      return { tone: "blue" as const, label: "部署中", detail: `已发现 ${matches.length} 个实例，当前存在进行中的部署任务。`, found: true };
    }
    if (failed.length > 0) {
      return { tone: "red" as const, label: "状态异常", detail: `已发现 ${matches.length} 个实例，请到服务与连接页面查看详情。`, found: true };
    }
    if (matches.some((service) => service.runtime === "paused")) {
      return { tone: "orange" as const, label: "已暂停", detail: `已发现 ${matches.length} 个实例，当前存在暂停的实例${versionText}。`, found: true };
    }
    if (matches.some((service) => service.runtime === "stopped")) {
      return { tone: "orange" as const, label: "已停止", detail: `已发现 ${matches.length} 个实例，当前没有运行中的实例${versionText}。`, found: true };
    }
    return { tone: "gray" as const, label: "状态未知", detail: `已发现 ${matches.length} 个实例，但暂时无法确认运行状态${versionText}。`, found: true };
  };

  return (
    <div className="page">
      <PageHeader title="应用目录" subtitle="包信息来自内置受信任清单，状态来自本机真实扫描结果。" />
      <div className="filter-tabs">
        <button className={kind === "all" ? "is-active" : ""} onClick={() => setKind("all")}>全部</button>
        <button className={kind === "windows_tool" ? "is-active" : ""} onClick={() => setKind("windows_tool")}>Windows 工具</button>
        <button className={kind === "container_app" ? "is-active" : ""} onClick={() => setKind("container_app")}>容器应用</button>
      </div>
      {items.length === 0 && <EmptyState title="没有可用包声明" detail="桌面后端尚未返回经过校验的本地包。" />}
      <div className="catalog-grid">
        {items.map((item) => {
          const state = packageState(item);
          return (
            <article className="catalog-card" key={item.id}>
              <div><span className="catalog-icon">{item.kind === "windows_tool" ? <Code2 /> : <Boxes />}</span><Badge tone={state.tone}>{state.label}</Badge></div>
              <h2>{item.name}</h2><strong>包声明版本：{item.version}</strong><p>{item.description}</p>
              <div className={`catalog-state catalog-state--${state.tone}`}>
                {state.tone === "green" ? <CheckCircle2 size={16} /> : state.tone === "blue" ? <LoaderCircle size={16} /> : <Info size={16} />}
                <span>{state.detail}</span>
              </div>
              <small>支持方式：{item.backends.map((value) => value === "compose" ? "独立 Compose" : value === "panel" ? "1Panel" : "Windows").join("、")}</small>
              {state.found
                ? <Button onClick={() => onView(item)}>{item.kind === "windows_tool" ? <Wrench size={16} /> : <Database size={16} />} {item.kind === "windows_tool" ? "查看开发环境" : "查看服务"}</Button>
                : item.id === "jenkins"
                  ? <Button
                      kind="primary"
                      onClick={() => onInstall(item)}
                      disabled={!runtimeReady}
                      loading={busyAction === `install:${item.id}`}
                      loadingLabel="正在创建任务"
                    ><PackagePlus size={16} /> 安装 Jenkins</Button>
                  : <div className="catalog-unavailable">当前包尚未开放真实安装流程。</div>}
            </article>
          );
        })}
      </div>
    </div>
  );
}

function TasksPage({ tasks }: { tasks: DeploymentTask[] }) {
  const [selectedId, setSelectedId] = useState("");
  const selected = tasks.find((task) => task.id === selectedId) ?? tasks[0];
  return (
    <div className="page">
      <PageHeader title="部署任务" subtitle="只展示后端实际创建并持久化的任务。" />
      <div className="tasks-layout">
        <section className="task-list">
          {tasks.length === 0 && <div className="empty"><CheckCircle2 /><strong>当前没有真实部署任务</strong></div>}
          {tasks.map((task) => <button className={selected?.id === task.id ? "is-active" : ""} key={task.id} onClick={() => setSelectedId(task.id)}><LoaderCircle /><span><strong>{task.title}</strong><small>{task.stage}</small></span><time>{task.updatedAt}</time></button>)}
        </section>
        <section className="task-detail">
          {selected ? <>
            <h2>{selected.title}</h2>
            <p>{selected.stage}</p>
            <div className="progress progress--large"><span style={{ width: `${selected.progress ?? 0}%` }} /></div>
            <pre>{selected.events.map((event) => `${event.time} [${event.level}] ${event.message}`).join("\n") || "[信息] 任务尚未产生事件"}</pre>
          </> : <div className="empty"><FileText /><strong>没有可查看的真实任务</strong></div>}
        </section>
      </div>
    </div>
  );
}

function SimplePage({
  page, snapshot, settings, storage, appVersion, runtimeReady, onBackendChange, onOpenStorage,
}: {
  page: PageKey;
  snapshot: WorkspaceSnapshot;
  settings: AppSettings;
  storage?: StorageLocations;
  appVersion: string;
  runtimeReady: boolean;
  onBackendChange: (backend: AppSettings["defaultDeploymentBackend"]) => void;
  onOpenStorage: (kind: "config" | "log") => void;
}) {
  if (page === "overview") return (
    <div className="page">
      <PageHeader title="本地开发工作台" subtitle="统计只来自桌面后端的真实资产。" />
      <div className="overview-grid">
        <section><Gauge /><h2>开发环境</h2><p>{snapshot.tools.length} 条实际检测记录，{snapshot.tools.filter((tool) => tool.configuration === "repair_required").length} 项配置待修复。</p></section>
        <section><Database /><h2>服务与连接</h2><p>{snapshot.services.length} 条真实服务记录，{snapshot.services.filter((service) => service.connection === "authenticated").length} 个连接完成协议鉴权。</p></section>
        <section><ClipboardList /><h2>部署任务</h2><p>{snapshot.tasks.filter((task) => task.status === "running").length} 个真实任务正在执行。</p></section>
      </div>
    </div>
  );
  if (page === "logs") return (
    <div className="page"><PageHeader title="诊断日志" subtitle="这里只汇总后端实际返回的通知，不生成示例日志。" />
      <section className="log-console">{snapshot.notices.length > 0 ? snapshot.notices.map((notice, index) => <div key={`${index}-${notice}`}>[信息] {notice}</div>) : <div>[信息] 当前没有后端诊断通知</div>}</section>
    </div>
  );
  return (
    <div className="page"><PageHeader title="设置" subtitle="设置和路径均来自当前用户的 LocalDock 桌面后端。" />
      <section className="settings-form">
        <h2>新部署计划默认目标</h2>
        <p>这是持久化的部署偏好，不会接管或改变已经存在的服务。</p>
        <label><input type="radio" checked={settings.defaultDeploymentBackend === "compose"} onChange={() => onBackendChange("compose")} disabled={!runtimeReady} name="backend" /> 独立 Compose</label>
        <label><input type="radio" checked={settings.defaultDeploymentBackend === "panel"} onChange={() => onBackendChange("panel")} disabled={!runtimeReady} name="backend" /> 1Panel 编排</label>
        <p className="settings-note">选择 1Panel 后，新建部署计划会以 1Panel 为首选目标；在写操作能力接入前，LocalDock 只执行发现和状态同步。</p>
        <hr />
        <h2>本地文件</h2>
        <p>服务元数据保存在当前用户目录，密码不会以明文写入配置文件。</p>
        <div className="storage-location">
          <span>配置文件</span>
          <code>{storage?.configFile ?? "正在读取..."}</code>
          <Button onClick={() => onOpenStorage("config")} disabled={!runtimeReady || !storage}><FolderOpen size={16} /> 打开配置目录</Button>
        </div>
        <div className="storage-location">
          <span>诊断日志</span>
          <code>{storage?.logFile ?? "正在读取..."}</code>
          <Button onClick={() => onOpenStorage("log")} disabled={!runtimeReady || !storage}><FolderOpen size={16} /> 打开日志目录</Button>
        </div>
        <hr />
        <div className="settings-version">
          <span>LocalDock 版本</span>
          <strong>{appVersion || "正在读取..."}</strong>
        </div>
      </section>
    </div>
  );
}

function AddServiceModal({
  onClose, onSaved,
}: {
  onClose: () => void;
  onSaved: (snapshot: WorkspaceSnapshot) => void;
}) {
  const [draft, setDraft] = useState<ConnectionDraft>({
    appId: "mysql",
    name: "MySQL",
    host: "127.0.0.1",
    port: 3306,
    tls: "disabled",
    remoteApproved: false,
    saveUnverified: true,
  });
  const [result, setResult] = useState<ConnectionProbeResult>();
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);

  const patchDraft = (patch: Partial<ConnectionDraft>) => {
    setDraft((current) => ({ ...current, ...patch }));
    setResult(undefined);
    setMessage("");
  };

  const runProbe = async () => {
    setBusy(true);
    setMessage("");
    try {
      setResult(await probeConnection(draft));
    } catch (reason) {
      setResult(undefined);
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const save = async () => {
    setBusy(true);
    setMessage("");
    try {
      onSaved(await saveExternalConnection(draft));
      onClose();
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal title="添加已有服务" onClose={onClose}>
      <div className="form-grid">
        <label>服务类型<select value={draft.appId} onChange={(event) => {
          const appId = event.target.value as ConnectionDraft["appId"];
          patchDraft({ appId, name: appId === "mysql" ? "MySQL" : appId === "redis" ? "Redis" : appId === "minio" ? "MinIO" : appId === "qdrant" ? "Qdrant" : "Jenkins", port: defaultPorts[appId] });
        }}><option value="mysql">MySQL</option><option value="redis">Redis</option><option value="minio">MinIO</option><option value="qdrant">Qdrant</option><option value="jenkins">Jenkins</option></select></label>
        <label>显示名称<input value={draft.name} onChange={(event) => patchDraft({ name: event.target.value })} /></label>
        <label>主机地址<input value={draft.host} onChange={(event) => patchDraft({ host: event.target.value })} /></label>
        <label>端口<input type="number" min="1" max="65535" value={draft.port} onChange={(event) => patchDraft({ port: Number(event.target.value) })} /></label>
        <label>用户名<input value={draft.username ?? ""} onChange={(event) => patchDraft({ username: event.target.value })} placeholder="仅保存元数据，不执行鉴权" /></label>
        <label>TLS 连接要求<select value={draft.tls} onChange={(event) => patchDraft({ tls: event.target.value as ConnectionDraft["tls"] })}><option value="disabled">未启用</option><option value="required">服务要求 TLS</option><option value="custom_ca">服务使用自定义证书</option></select></label>
        <label className="form-span"><input type="checkbox" checked={draft.remoteApproved} onChange={(event) => patchDraft({ remoteApproved: event.target.checked })} /> 我确认允许连接非本机地址</label>
        <label className="form-span"><input type="checkbox" checked={draft.saveUnverified} onChange={(event) => patchDraft({ saveUnverified: event.target.checked })} /> 端口不可达时仍保存为未验证连接</label>
      </div>
      <div className="form-note"><Info size={16} /> 当前测试只执行真实 TCP 端口探测，不会声称协议鉴权或健康检查通过。</div>
      {result && <div className="test-result"><Info /> {result.summary} 检查时间：{result.checkedAt}</div>}
      {message && <div className="modal-error"><Info /> {message}</div>}
      <footer><Button onClick={runProbe} disabled={busy}><Play size={16} /> {busy ? "正在探测" : "测试端口可达性"}</Button><Button kind="primary" onClick={save} disabled={busy}><CheckCircle2 size={16} /> 保存真实记录</Button></footer>
    </Modal>
  );
}

export default function App() {
  const [snapshot, setSnapshot] = useState<WorkspaceSnapshot>(emptyWorkspace);
  const [page, setPage] = useState<PageKey>("services");
  const [selectedId, setSelectedId] = useState("");
  const [busyAction, setBusyAction] = useState("");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [showAdd, setShowAdd] = useState(false);
  const [settings, setSettings] = useState<AppSettings>({ defaultDeploymentBackend: "compose" });
  const [storage, setStorage] = useState<StorageLocations>();
  const [appVersion, setAppVersion] = useState("");
  const runtime = isTauriRuntime();
  const selected = snapshot.services.find((service) => service.id === selectedId);
  const hasActiveTask = snapshot.tasks.some((task) => task.status === "running" || task.status === "queued");

  useEffect(() => {
    if (!runtime) return;
    Promise.all([loadWorkspace(), getAppSettings(), getStorageLocations(), getAppVersion()])
      .then(([nextSnapshot, nextSettings, nextStorage, nextVersion]) => {
        setSnapshot(nextSnapshot);
        setSettings(nextSettings);
        setStorage(nextStorage);
        setAppVersion(nextVersion);
      })
      .catch((reason: unknown) => setError(`读取本地数据失败：${String(reason)}`));
  }, [runtime]);

  useEffect(() => {
    if (!runtime || !hasActiveTask) return;
    const timer = window.setInterval(() => {
      loadWorkspace()
        .then(setSnapshot)
        .catch((reason: unknown) => setError(`刷新部署进度失败：${String(reason)}`));
    }, 1400);
    return () => window.clearInterval(timer);
  }, [runtime, hasActiveTask]);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(""), 2600);
    return () => window.clearTimeout(timer);
  }, [notice]);

  const perform = async (
    action: () => Promise<WorkspaceSnapshot>,
    actionKey: string,
    successMessage: string,
  ) => {
    setBusyAction(actionKey);
    setError("");
    setNotice("");
    try {
      setSnapshot(await action());
      setNotice(successMessage);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusyAction("");
    }
  };

  const performVoid = async (action: () => Promise<void>, actionKey: string, successMessage: string) => {
    setBusyAction(actionKey);
    setError("");
    setNotice("");
    try {
      await action();
      setNotice(successMessage);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusyAction("");
    }
  };

  const handleSelect = (service: ServiceRecord) => {
    setSelectedId((current) => current === service.id ? "" : service.id);
  };

  const handlePackageInstall = async (item: CatalogPackage) => {
    if (!runtime) {
      setError("当前未连接桌面后端，不能安装应用。");
      return;
    }
    const actionKey = `install:${item.id}`;
    setBusyAction(actionKey);
    setError("");
    setNotice("");
    try {
      setSnapshot(await startPackageInstall(item.id));
      setPage("tasks");
      setNotice(`${item.name} 部署任务已创建，可在任务页查看真实进度。`);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusyAction("");
    }
  };

  const copyJenkinsInitialPassword = async (service: ServiceRecord) => {
    const actionKey = `jenkins-password:${service.id}`;
    setBusyAction(actionKey);
    setError("");
    try {
      const password = await getJenkinsInitialPassword(service.id);
      await writeClipboardText(password);
      setNotice("Jenkins 初始管理员密码已复制，LocalDock 未保存该密码。");
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusyAction("");
    }
  };

  const runWindowAction = async (action: "minimize" | "maximize" | "close") => {
    if (!runtime) {
      setError("当前未运行在桌面窗口中，无法执行窗口操作。");
      return;
    }

    try {
      const currentWindow = getCurrentWindow();
      if (action === "minimize") await currentWindow.minimize();
      if (action === "maximize") await currentWindow.toggleMaximize();
      if (action === "close") await currentWindow.close();
    } catch (reason) {
      setError(`窗口操作失败：${String(reason)}`);
    }
  };

  const handleTitlebarMouseDown = async (event: React.MouseEvent<HTMLElement>) => {
    if (!runtime || event.button !== 0 || (event.target as HTMLElement).closest("button")) return;

    try {
      const currentWindow = getCurrentWindow();
      if (event.detail === 2) {
        await currentWindow.toggleMaximize();
      } else {
        await currentWindow.startDragging();
      }
    } catch (reason) {
      setError(`窗口操作失败：${String(reason)}`);
    }
  };

  const taskCount = useMemo(
    () => snapshot.tasks.filter((task) => task.status === "running" || task.status === "queued").length,
    [snapshot.tasks],
  );

  return (
    <div className="desktop-shell">
      <header className="titlebar" onMouseDown={handleTitlebarMouseDown}>
        <div><Logo /><strong>LocalDock</strong></div>
        <div className="window-controls">
          <button type="button" title="最小化" aria-label="最小化" onClick={() => runWindowAction("minimize")}><Minus size={17} /></button>
          <button type="button" title="最大化或还原" aria-label="最大化或还原" onClick={() => runWindowAction("maximize")}><Square size={14} /></button>
          <button type="button" title="关闭" aria-label="关闭" onClick={() => runWindowAction("close")}><X size={18} /></button>
        </div>
      </header>
      <div className="workspace">
        <aside className="sidebar">
          <nav>{navItems.map((item) => {
            const Icon = item.icon;
            return <button key={item.key} className={page === item.key ? "is-active" : ""} onClick={() => setPage(item.key)}><Icon size={20} /><span>{item.label}</span>{item.key === "tasks" && taskCount > 0 && <b>{taskCount}</b>}</button>;
          })}</nav>
          <div className="sidebar-footer"><span className={`status-light ${runtime ? "" : "status-light--offline"}`} /> {runtime ? "桌面真实模式" : "后端未连接"}<small>{runtime ? "数据来自本机桌面后端" : "不展示模拟业务数据"}</small></div>
        </aside>
        <section className="content">
          {error && <div className="error-banner"><Info size={17} /> {error}<button onClick={() => setError("")}>关闭</button></div>}
          {notice && <div className="feedback-toast" role="status"><CheckCircle2 size={18} /> {notice}</div>}
          {page === "services" && <ServicesPage
            snapshot={snapshot}
            selected={selected}
            busyAction={busyAction}
            runtimeReady={runtime}
            onScan={() => runtime ? perform(scanLocalMachine, "scan", "本机环境扫描完成，结果已更新。") : setError("当前未连接桌面后端，不能扫描本机。")}
            onSelect={handleSelect}
            onTest={(service) => runtime ? perform(() => testConnection(service.id), `test:${service.id}`, `${service.name} 端口探测完成。`) : setError("当前未连接桌面后端，不能执行连接测试。")}
            onOpen={(service) => runtime ? performVoid(() => openServiceEndpoint(service.id), `open:${service.id}`, `已打开 ${service.name} 的网页入口。`) : setError("当前未连接桌面后端，不能打开服务入口。")}
            onAdd={() => runtime ? setShowAdd(true) : setError("当前未连接桌面后端，不能保存服务记录。")}
            onInstall={() => setPage("catalog")}
            onCancel={(taskId) => runtime ? perform(() => cancelTask(taskId), `cancel:${taskId}`, "任务已标记为取消。") : setError("当前未连接桌面后端，不能取消任务。")}
            onCopySetupPassword={copyJenkinsInitialPassword}
          />}
          {page === "tools" && <ToolsPage
            tools={snapshot.tools}
            runtimeReady={runtime}
            busyAction={busyAction}
            onScan={() => runtime ? perform(scanLocalMachine, "scan", "开发环境检测完成，版本和路径已更新。") : setError("当前未连接桌面后端，不能扫描 Windows 工具。")}
          />}
          {page === "catalog" && <CatalogPage
            packages={snapshot.packages}
            services={snapshot.services}
            tools={snapshot.tools}
            runtimeReady={runtime}
            busyAction={busyAction}
            onInstall={handlePackageInstall}
            onView={(item) => {
              if (item.kind === "windows_tool") {
                setPage("tools");
                setNotice(`已切换到开发环境，可查看 ${item.name} 的真实检测结果。`);
                return;
              }
              const service = snapshot.services.find((candidate) => candidate.appId === item.id);
              if (service) setSelectedId(service.id);
              setPage("services");
              setNotice(`已切换到服务与连接，可查看 ${item.name} 的真实运行状态。`);
            }}
          />}
          {page === "tasks" && <TasksPage tasks={snapshot.tasks} />}
          {page !== "services" && page !== "tools" && page !== "catalog" && page !== "tasks" && <SimplePage
            page={page}
            snapshot={snapshot}
            settings={settings}
            storage={storage}
            appVersion={appVersion}
            runtimeReady={runtime}
            onBackendChange={(backend) => {
              if (!runtime) {
                setError("当前未连接桌面后端，不能保存部署方式。");
                return;
              }
              setError("");
              setDefaultDeploymentBackend(backend)
                .then((next) => {
                  setSettings(next);
                  setNotice(`默认部署方式已保存为${backend === "panel" ? " 1Panel 编排" : "独立 Compose"}。`);
                })
                .catch((reason: unknown) => setError(`保存默认部署方式失败：${String(reason)}`));
            }}
            onOpenStorage={(kind) => runtime
              ? performVoid(() => openStorageDirectory(kind), `open-storage:${kind}`, kind === "config" ? "已打开配置文件目录。" : "已打开诊断日志目录。")
              : setError("当前未连接桌面后端，不能打开本地目录。")}
          />}
        </section>
      </div>
      {showAdd && <AddServiceModal onClose={() => setShowAdd(false)} onSaved={(next) => {
        setSnapshot(next);
        setSelectedId(next.services[next.services.length - 1]?.id ?? "");
      }} />}
    </div>
  );
}
