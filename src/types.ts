export type ManagementKind =
  | "localdock_compose"
  | "1panel_compose"
  | "external_connection"
  | "external_container"
  | "localdock_windows_tool"
  | "external_windows_tool";

export type ConnectionState = "unknown" | "reachable" | "authenticated" | "failed";
export type RuntimeState = "unknown" | "running" | "paused" | "stopped" | "installing" | "error";
export type HealthState = "healthy" | "unhealthy" | "unverified" | "not_applicable";
export type Capability =
  | "connect"
  | "test"
  | "copy_connection"
  | "open_browser"
  | "start"
  | "stop"
  | "restart"
  | "read_logs";

export interface ServiceRecord {
  id: string;
  appId: "mysql" | "redis" | "minio" | "qdrant" | "jenkins";
  name: string;
  version?: string;
  target: string;
  host: string;
  port: number;
  username?: string;
  database?: string;
  tls: "disabled" | "required" | "custom_ca";
  management: ManagementKind;
  connection: ConnectionState;
  runtime: RuntimeState;
  endpointPublished: boolean;
  health: HealthState;
  credentialState: "encrypted" | "missing" | "not_required";
  capabilities: Capability[];
  lastChecked?: string;
  taskId?: string;
  progress?: number;
  progressLabel?: string;
  distribution?: string;
  composePath?: string;
  composeProject?: string;
  containerId?: string;
}

export interface PanelState {
  state: "not_found" | "authorization_required" | "read_only_discovered" | "authorized" | "error";
  endpoint?: string;
  message: string;
}

export interface ToolRecord {
  id: string;
  name: string;
  installedVersions: string[];
  activeVersion?: string;
  configuration: "ready" | "repair_required" | "not_installed";
  management: ManagementKind;
  path?: string;
}

export interface TaskEvent {
  sequence: number;
  level: "信息" | "成功" | "警告" | "错误";
  message: string;
  time: string;
}

export interface DeploymentTask {
  id: string;
  instanceId?: string;
  title: string;
  stage: string;
  status: "queued" | "running" | "failed" | "completed" | "cancelled";
  progress?: number;
  transferred?: string;
  total?: string;
  updatedAt: string;
  events: TaskEvent[];
}

export interface CatalogPackage {
  id: string;
  kind: "windows_tool" | "container_app";
  name: string;
  version: string;
  category: string;
  description: string;
  backends: Array<"windows" | "compose" | "panel">;
  trusted: boolean;
}

export interface WorkspaceSnapshot {
  mode: "real" | "unavailable";
  scannedAt: string;
  services: ServiceRecord[];
  tools: ToolRecord[];
  tasks: DeploymentTask[];
  panel: PanelState;
  packages: CatalogPackage[];
  notices: string[];
}

export interface AppSettings {
  defaultDeploymentBackend: "compose" | "panel";
}

export interface StorageLocations {
  configFile: string;
  logFile: string;
}

export interface ConnectionDraft {
  appId: ServiceRecord["appId"];
  name: string;
  host: string;
  port: number;
  username?: string;
  database?: string;
  tls: ServiceRecord["tls"];
  password?: string;
  remoteApproved: boolean;
  saveUnverified: boolean;
}

export interface ConnectionProbeResult {
  state: "reachable" | "failed";
  summary: string;
  checkedAt: string;
}
