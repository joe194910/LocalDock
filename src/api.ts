import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings, ConnectionDraft, ConnectionProbeResult, StorageLocations, UpdateCheckResult,
  WorkspaceSnapshot,
} from "./types";

export const isTauriRuntime = () => "__TAURI_INTERNALS__" in window;

export async function loadWorkspace(): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>("load_workspace");
}

export async function getAppVersion(): Promise<string> {
  return invoke<string>("get_app_version");
}

export async function checkForUpdates(): Promise<UpdateCheckResult> {
  return invoke<UpdateCheckResult>("check_for_updates");
}

export async function openReleasesPage(): Promise<void> {
  await invoke("open_releases_page");
}

export async function startPackageInstall(packageId: string): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>("start_package_install", { packageId });
}

export async function scanLocalMachine(): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>("scan_local_machine");
}

export async function testConnection(serviceId: string): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>("test_connection", { serviceId });
}

export async function probeConnection(draft: ConnectionDraft): Promise<ConnectionProbeResult> {
  return invoke<ConnectionProbeResult>("probe_connection", { draft });
}

export async function saveExternalConnection(draft: ConnectionDraft): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>("save_external_connection", { draft });
}

export async function openServiceEndpoint(serviceId: string): Promise<void> {
  await invoke("open_service_endpoint", { serviceId });
}

export async function getJenkinsInitialPassword(serviceId: string): Promise<string> {
  return invoke<string>("get_jenkins_initial_password", { serviceId });
}

export async function cancelTask(taskId: string): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>("cancel_task", { taskId });
}

export async function runServiceAction(
  serviceId: string,
  action: "start" | "stop" | "restart",
): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>("run_service_action", { serviceId, action });
}

export async function getAppSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("get_app_settings");
}

export async function setDefaultDeploymentBackend(
  backend: AppSettings["defaultDeploymentBackend"],
): Promise<AppSettings> {
  return invoke<AppSettings>("set_default_deployment_backend", { backend });
}

export async function getStorageLocations(): Promise<StorageLocations> {
  return invoke<StorageLocations>("get_storage_locations");
}

export async function openStorageDirectory(kind: "config" | "log"): Promise<void> {
  await invoke("open_storage_directory", { kind });
}
