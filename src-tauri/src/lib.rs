use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: u32 = 3;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceRecord {
    id: String,
    app_id: String,
    name: String,
    version: Option<String>,
    target: String,
    host: String,
    port: u16,
    username: Option<String>,
    database: Option<String>,
    tls: String,
    management: String,
    connection: String,
    runtime: String,
    #[serde(default = "default_true")]
    endpoint_published: bool,
    health: String,
    credential_state: String,
    capabilities: Vec<String>,
    last_checked: Option<String>,
    task_id: Option<String>,
    progress: Option<u8>,
    progress_label: Option<String>,
    #[serde(default)]
    distribution: Option<String>,
    #[serde(default)]
    compose_path: Option<String>,
    #[serde(default)]
    compose_project: Option<String>,
    #[serde(default)]
    container_id: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PanelState {
    state: String,
    endpoint: Option<String>,
    message: String,
}

struct WslScanResult {
    panel: PanelState,
    services: Vec<ServiceRecord>,
    notices: Vec<String>,
}

#[derive(Deserialize)]
struct DockerInspect {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "State")]
    state: DockerState,
    #[serde(rename = "Config")]
    config: DockerConfig,
    #[serde(rename = "NetworkSettings")]
    network_settings: DockerNetworkSettings,
}

#[derive(Deserialize)]
struct DockerState {
    #[serde(rename = "Status")]
    status: String,
}

#[derive(Deserialize)]
struct DockerConfig {
    #[serde(rename = "Image")]
    image: String,
    #[serde(rename = "Labels")]
    labels: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
struct DockerNetworkSettings {
    #[serde(rename = "Ports")]
    ports: Option<HashMap<String, Option<Vec<DockerPortBinding>>>>,
}

#[derive(Deserialize)]
struct DockerPortBinding {
    #[serde(rename = "HostIp")]
    host_ip: String,
    #[serde(rename = "HostPort")]
    host_port: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolRecord {
    id: String,
    name: String,
    installed_versions: Vec<String>,
    active_version: Option<String>,
    configuration: String,
    management: String,
    path: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskEvent {
    sequence: u64,
    level: String,
    message: String,
    time: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeploymentTask {
    id: String,
    instance_id: Option<String>,
    title: String,
    stage: String,
    status: String,
    progress: Option<u8>,
    transferred: Option<String>,
    total: Option<String>,
    updated_at: String,
    events: Vec<TaskEvent>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogPackage {
    id: String,
    kind: String,
    name: String,
    version: String,
    category: String,
    description: String,
    backends: Vec<String>,
    trusted: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageManifest {
    schema_version: u32,
    id: String,
    kind: String,
    name: String,
    version: String,
    trusted_source: bool,
    capabilities: Vec<String>,
    image_repository: Option<String>,
    image_tag: Option<String>,
    image_variable: Option<String>,
    compose_template: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceSnapshot {
    mode: String,
    scanned_at: String,
    services: Vec<ServiceRecord>,
    tools: Vec<ToolRecord>,
    tasks: Vec<DeploymentTask>,
    panel: PanelState,
    packages: Vec<CatalogPackage>,
    notices: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    default_deployment_backend: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            default_deployment_backend: "compose".to_string(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StorageLocations {
    config_file: String,
    log_file: String,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    html_url: String,
    published_at: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateCheckResult {
    current_version: String,
    latest_version: String,
    update_available: bool,
    release_name: String,
    release_url: String,
    published_at: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssetDocument {
    schema_version: u32,
    snapshot: WorkspaceSnapshot,
    #[serde(default)]
    settings: AppSettings,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionDraft {
    app_id: String,
    name: String,
    host: String,
    port: u16,
    username: Option<String>,
    database: Option<String>,
    tls: String,
    password: Option<String>,
    remote_approved: bool,
    save_unverified: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionProbeResult {
    state: String,
    summary: String,
    checked_at: String,
}

trait ContainerBackend {
    fn kind(&self) -> &'static str;
    fn capabilities(&self, service: &ServiceRecord) -> Vec<String>;
    fn run_action(&self, service: &ServiceRecord, action: &str) -> Result<(), String>;
}

struct ComposeBackend;
struct PanelBackend;

impl ContainerBackend for ComposeBackend {
    fn kind(&self) -> &'static str {
        "localdock_compose"
    }

    fn capabilities(&self, service: &ServiceRecord) -> Vec<String> {
        service.capabilities.clone()
    }

    fn run_action(&self, service: &ServiceRecord, action: &str) -> Result<(), String> {
        if !matches!(action, "start" | "stop" | "restart") {
            return Err("不支持的服务操作。".to_string());
        }
        let required = action.to_string();
        if !self.capabilities(service).contains(&required) {
            return Err(format!("当前实例不具备“{}”管理能力。", action_name(action)));
        }
        let distribution = service.distribution.as_deref()
            .ok_or_else(|| "当前实例缺少 WSL 发行版信息，不能执行管理操作。".to_string())?;
        let compose_path = service.compose_path.as_deref()
            .ok_or_else(|| "当前实例缺少 Compose 项目目录，不能执行管理操作。".to_string())?;
        let compose_project = service.compose_project.as_deref()
            .ok_or_else(|| "当前实例缺少 Compose 项目名称，不能执行管理操作。".to_string())?;
        let compose_file = format!("{}/compose.yaml", compose_path);
        let env_file = format!("{}/.env", compose_path);
        wsl_output(
            distribution,
            &[
                "docker", "compose", "-p", compose_project,
                "--project-directory", compose_path,
                "--env-file", env_file.as_str(),
                "-f", compose_file.as_str(),
                action,
            ],
        ).map(|_| ())
            .map_err(|error| format!("执行 Compose {}操作失败：{}", action_name(action), error))
    }
}

impl ContainerBackend for PanelBackend {
    fn kind(&self) -> &'static str {
        "1panel_compose"
    }

    fn capabilities(&self, service: &ServiceRecord) -> Vec<String> {
        service.capabilities.clone()
    }

    fn run_action(&self, service: &ServiceRecord, action: &str) -> Result<(), String> {
        if !self.capabilities(service).contains(&action.to_string()) {
            return Err("1Panel 尚未授权该管理能力。".to_string());
        }
        Err("尚未配置经过核验的 1Panel API 凭据，不能绕过面板直接修改容器。".to_string())
    }
}

fn action_name(action: &str) -> &'static str {
    match action {
        "start" => "启动",
        "stop" => "停止",
        "restart" => "重启",
        _ => "未知",
    }
}

fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn now_text() -> String {
    format!("Unix {}", unix_time())
}

fn data_dir() -> Result<PathBuf, String> {
    let base = env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| "无法读取当前用户的 LOCALAPPDATA 目录。".to_string())?;
    Ok(base.join("LocalDock"))
}

fn asset_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("assets.json"))
}

fn diagnostic_log_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("diagnostics").join("localdock.log"))
}

fn append_diagnostic_log(message: &str) -> Result<(), String> {
    let path = diagnostic_log_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建诊断日志目录失败：{}", error))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("打开诊断日志失败：{}", error))?;
    writeln!(file, "{} [信息] {}", now_text(), message)
        .map_err(|error| format!("写入诊断日志失败：{}", error))
}

fn default_packages() -> Vec<CatalogPackage> {
    embedded_package_manifests()
        .into_iter()
        .map(catalog_from_manifest)
        .collect()
}

fn embedded_package_manifests() -> [&'static str; 9] {
    [
        include_str!("../../tool-packages/temurin-jdk/manifest.json"),
        include_str!("../../tool-packages/node/manifest.json"),
        include_str!("../../tool-packages/maven/manifest.json"),
        include_str!("../../tool-packages/git/manifest.json"),
        include_str!("../../app-packages/mysql/manifest.json"),
        include_str!("../../app-packages/redis/manifest.json"),
        include_str!("../../app-packages/minio/manifest.json"),
        include_str!("../../app-packages/qdrant/manifest.json"),
        include_str!("../../app-packages/jenkins/manifest.json"),
    ]
}

fn catalog_from_manifest(source: &str) -> CatalogPackage {
    let manifest: PackageManifest = serde_json::from_str(source)
        .unwrap_or_else(|error| panic!("内置应用包清单无效：{}", error));
    assert_eq!(manifest.schema_version, 1, "内置应用包清单版本不受支持");
    let backends = manifest.capabilities.into_iter()
        .filter(|capability| matches!(capability.as_str(), "windows" | "compose" | "panel"))
        .collect::<Vec<_>>();
    CatalogPackage {
        id: manifest.id,
        category: if manifest.kind == "windows_tool" { "Windows 工具" } else { "容器应用" }.to_string(),
        description: "LocalDock 内置包清单；版本和能力在构建时经过结构校验。".to_string(),
        kind: manifest.kind,
        name: manifest.name,
        version: manifest.version,
        backends,
        trusted: manifest.trusted_source,
    }
}

fn empty_snapshot() -> WorkspaceSnapshot {
    WorkspaceSnapshot {
        mode: "real".to_string(),
        scanned_at: now_text(),
        services: Vec::new(),
        tools: Vec::new(),
        tasks: Vec::new(),
        panel: PanelState {
            state: "not_found".to_string(),
            endpoint: None,
            message: "未发现 1Panel；这不影响本机工具、外部连接和独立 Compose 部署。".to_string(),
        },
        packages: default_packages(),
        notices: Vec::new(),
    }
}

fn load_document() -> Result<AssetDocument, String> {
    let path = asset_path()?;
    if !path.exists() {
        return Ok(AssetDocument {
            schema_version: SCHEMA_VERSION,
            snapshot: empty_snapshot(),
            settings: AppSettings::default(),
        });
    }
    let mut input = String::new();
    fs::File::open(&path)
        .and_then(|mut file| file.read_to_string(&mut input))
        .map_err(|error| format!("读取资产文件失败：{}", error))?;
    let mut document: AssetDocument = serde_json::from_str(&input)
        .map_err(|error| format!("资产文件格式无效，原文件未被覆盖：{}", error))?;
    if document.schema_version > SCHEMA_VERSION {
        return Err("资产文件版本高于当前应用支持版本，请先升级 LocalDock。".to_string());
    }
    let mut changed = false;
    if document.schema_version < SCHEMA_VERSION {
        document.schema_version = SCHEMA_VERSION;
        document.snapshot.notices.push("已保留旧资产并完成结构迁移。".to_string());
        changed = true;
    }
    let packages = default_packages();
    if document.snapshot.packages != packages {
        document.snapshot.packages = packages;
        changed = true;
    }
    for tool in &mut document.snapshot.tools {
        if tool.id == "jdk" {
            tool.id = "temurin-jdk".to_string();
            changed = true;
        }
    }
    if changed {
        save_document(&document)?;
    }
    Ok(document)
}

fn save_document(document: &AssetDocument) -> Result<(), String> {
    let path = asset_path()?;
    let directory = path.parent().ok_or_else(|| "资产目录无效。".to_string())?;
    fs::create_dir_all(directory).map_err(|error| format!("创建资产目录失败：{}", error))?;
    let lock_path = directory.join("assets.lock");
    let _lock = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .map_err(|_| "另一个 LocalDock 实例正在写入资产，请稍后重试。".to_string())?;
    let result = save_document_locked(document, &path);
    let _ = fs::remove_file(lock_path);
    result
}

fn save_document_locked(document: &AssetDocument, path: &Path) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(document).map_err(|error| format!("序列化资产失败：{}", error))?;
    let temporary = path.with_extension("json.tmp");
    let backup = path.with_extension("json.bak");
    {
        let mut file = fs::File::create(&temporary).map_err(|error| format!("创建临时资产文件失败：{}", error))?;
        file.write_all(&bytes).map_err(|error| format!("写入临时资产文件失败：{}", error))?;
        file.sync_all().map_err(|error| format!("同步临时资产文件失败：{}", error))?;
    }
    if path.exists() {
        fs::copy(path, &backup).map_err(|error| format!("创建资产备份失败：{}", error))?;
        fs::remove_file(path).map_err(|error| format!("替换旧资产文件失败：{}", error))?;
    }
    fs::rename(&temporary, path).map_err(|error| format!("启用新资产文件失败：{}", error))
}

fn background_command(program: &str) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

fn command_text(program: &str, arguments: &[&str]) -> Option<String> {
    let output = background_command(program)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let error_text = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if text.is_empty() {
        text = error_text;
    } else if !error_text.is_empty() {
        text.push('\n');
        text.push_str(&error_text);
    }
    (!text.is_empty()).then_some(text)
}

fn decode_process_output(bytes: &[u8]) -> String {
    if bytes.chunks_exact(2).take(32).any(|pair| pair[1] == 0) {
        let words = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        String::from_utf16_lossy(&words)
            .trim_start_matches('\u{feff}')
            .to_string()
    } else {
        String::from_utf8_lossy(bytes).to_string()
    }
}

fn command_output(program: &str, arguments: &[&str]) -> Result<String, String> {
    let output = background_command(program)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("执行系统命令失败：{}", error))?;
    if !output.status.success() {
        let error_text = decode_process_output(&output.stderr).trim().to_string();
        return Err(if error_text.is_empty() {
            format!("系统命令执行失败，退出码：{:?}", output.status.code())
        } else {
            error_text
        });
    }
    Ok(decode_process_output(&output.stdout).trim().to_string())
}

fn parse_version(version: &str) -> Result<Vec<u32>, String> {
    let trimmed = version.trim();
    let normalized = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .unwrap_or(trimmed);
    let core = normalized
        .split(['-', '+'])
        .next()
        .ok_or_else(|| format!("版本号“{}”格式无效", version))?;
    let parts = core
        .split('.')
        .map(|part| {
            part.parse::<u32>()
                .map_err(|_| format!("版本号“{}”格式无效", version))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if parts.is_empty() {
        return Err(format!("版本号“{}”格式无效", version));
    }
    Ok(parts)
}

fn is_newer_version(latest: &str, current: &str) -> Result<bool, String> {
    let mut latest_parts = parse_version(latest)?;
    let mut current_parts = parse_version(current)?;
    let length = latest_parts.len().max(current_parts.len());
    latest_parts.resize(length, 0);
    current_parts.resize(length, 0);
    Ok(latest_parts > current_parts)
}

fn scan_tool(id: &str, name: &str, executable: &str, version_args: &[&str]) -> ToolRecord {
    let path = command_text("where.exe", &[executable])
        .and_then(|text| text.lines().next().map(str::to_string));
    let version = path.as_ref().and_then(|_| command_text(executable, version_args))
        .map(|text| text.lines().next().unwrap_or(&text).trim().to_string());
    ToolRecord {
        id: id.to_string(),
        name: name.to_string(),
        installed_versions: version.clone().into_iter().collect(),
        active_version: version,
        configuration: if path.is_some() { "ready" } else { "not_installed" }.to_string(),
        management: "external_windows_tool".to_string(),
        path,
    }
}

fn scan_tools() -> Vec<ToolRecord> {
    vec![
        scan_tool("temurin-jdk", "Eclipse Temurin JDK", "java.exe", &["-version"]),
        scan_tool("node", "Node.js", "node.exe", &["--version"]),
        scan_tool("maven", "Apache Maven", "mvn.cmd", &["--version"]),
        scan_tool("git", "Git for Windows", "git.exe", &["--version"]),
    ]
}

fn running_wsl_distributions() -> Result<Vec<String>, String> {
    let output = command_output("wsl.exe", &["--list", "--running", "--quiet"])?;
    Ok(output
        .lines()
        .map(|line| line.trim().trim_matches('\0'))
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

fn wsl_output(distribution: &str, arguments: &[&str]) -> Result<String, String> {
    let mut command_arguments = vec!["-d", distribution, "-u", "root", "--"];
    command_arguments.extend_from_slice(arguments);
    command_output("wsl.exe", &command_arguments)
}

fn panel_status(distribution: &str) -> Option<bool> {
    let script = "if command -v 1pctl >/dev/null 2>&1 || [ -x /usr/bin/1panel ] || systemctl cat 1panel >/dev/null 2>&1; then if systemctl is-active --quiet 1panel 2>/dev/null || pgrep -x 1panel >/dev/null 2>&1; then printf running; else printf stopped; fi; fi";
    match wsl_output(distribution, &["sh", "-c", script]) {
        Ok(output) if output == "running" => Some(true),
        Ok(output) if output == "stopped" => Some(false),
        _ => None,
    }
}

fn docker_inspects(distribution: &str) -> Result<Vec<DockerInspect>, String> {
    let docker_available = wsl_output(
        distribution,
        &["sh", "-c", "if command -v docker >/dev/null 2>&1; then printf yes; fi"],
    )?;
    if docker_available != "yes" {
        return Ok(Vec::new());
    }
    let ids = wsl_output(distribution, &["docker", "ps", "-aq"])?;
    if ids.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut arguments = vec!["docker", "inspect"];
    let container_ids = ids.lines().map(str::trim).filter(|id| !id.is_empty()).collect::<Vec<_>>();
    arguments.extend(container_ids);
    let output = wsl_output(distribution, &arguments)?;
    serde_json::from_str(&output).map_err(|error| format!("解析 Docker 容器信息失败：{}", error))
}

fn known_app(container: &DockerInspect) -> Option<(&'static str, &'static str, u16)> {
    let labels = container.config.labels.as_ref();
    let evidence = format!(
        "{} {} {} {}",
        container.name,
        container.config.image,
        labels.and_then(|items| items.get("com.docker.compose.project")).map(String::as_str).unwrap_or_default(),
        labels.and_then(|items| items.get("com.docker.compose.service")).map(String::as_str).unwrap_or_default(),
    ).to_ascii_lowercase();
    if evidence.contains("mysql") {
        Some(("mysql", "MySQL", 3306))
    } else if evidence.contains("redis") {
        Some(("redis", "Redis", 6379))
    } else if evidence.contains("minio") {
        Some(("minio", "MinIO", 9001))
    } else if evidence.contains("qdrant") {
        Some(("qdrant", "Qdrant", 6333))
    } else if evidence.contains("jenkins") {
        Some(("jenkins", "Jenkins", 8080))
    } else {
        None
    }
}

fn published_endpoint(container: &DockerInspect, preferred_port: u16) -> Option<(String, u16)> {
    let preferred_key = format!("{}/tcp", preferred_port);
    let ports = container.network_settings.ports.as_ref()?;
    let binding = ports.get(&preferred_key)
        .and_then(Option::as_ref)
        .and_then(|items| items.first())
        .or_else(|| {
            ports.values()
                .filter_map(Option::as_ref)
                .find_map(|items| items.first())
        })?;
    let port = binding.host_port.parse::<u16>().ok()?;
    let host = match binding.host_ip.as_str() {
        "" | "0.0.0.0" | "::" | "::0" => "127.0.0.1".to_string(),
        value => value.to_string(),
    };
    Some((host, port))
}

fn image_version(image: &str) -> Option<String> {
    let without_digest = image.split('@').next().unwrap_or(image);
    let slash = without_digest.rfind('/').unwrap_or(0);
    let colon = without_digest.rfind(':')?;
    (colon > slash).then(|| without_digest[colon + 1..].to_string())
}

fn is_panel_container(container: &DockerInspect) -> bool {
    let Some(labels) = container.config.labels.as_ref() else {
        return false;
    };
    if labels.get("createdBy").is_some_and(|value| value.eq_ignore_ascii_case("Apps")) {
        return true;
    }
    ["com.docker.compose.project.config_files", "com.docker.compose.project.working_dir"]
        .iter()
        .filter_map(|key| labels.get(*key))
        .any(|value| value.to_ascii_lowercase().contains("1panel"))
}

fn service_from_container(distribution: &str, container: DockerInspect) -> Option<ServiceRecord> {
    let (app_id, app_name, preferred_port) = known_app(&container)?;
    let endpoint = published_endpoint(&container, preferred_port);
    let endpoint_published = endpoint.is_some();
    let (host, port) = endpoint.unwrap_or_else(|| ("127.0.0.1".to_string(), preferred_port));
    let labels = container.config.labels.as_ref();
    let localdock_managed = labels
        .and_then(|items| items.get("cn.localdock.managed"))
        .is_some_and(|value| value.eq_ignore_ascii_case("true"));
    let management = if localdock_managed {
        "localdock_compose"
    } else if is_panel_container(&container) {
        "1panel_compose"
    } else {
        "external_container"
    };
    let compose_path = labels
        .and_then(|items| items.get("com.docker.compose.project.working_dir"))
        .cloned();
    let compose_project = labels
        .and_then(|items| items.get("com.docker.compose.project"))
        .cloned();
    let runtime = match container.state.status.as_str() {
        "running" => "running",
        "paused" => "paused",
        "exited" | "created" | "dead" | "removing" => "stopped",
        _ => "unknown",
    };
    let connection = if endpoint_published
        && runtime == "running"
        && TcpStream::connect_timeout(
            &resolve_target(&host, port).ok()?,
            Duration::from_millis(700),
        ).is_ok()
    {
        "reachable"
    } else {
        "unknown"
    };
    let mut capabilities = Vec::new();
    if endpoint_published {
        capabilities.extend([
            "connect".to_string(),
            "test".to_string(),
            "copy_connection".to_string(),
        ]);
    }
    if endpoint_published && matches!(app_id, "minio" | "qdrant" | "jenkins") {
        capabilities.push("open_browser".to_string());
    }
    if localdock_managed {
        capabilities.extend(["start", "stop", "restart", "read_logs"].map(str::to_string));
    }
    let short_id = container.id.chars().take(12).collect::<String>();
    let distribution_id = distribution.chars()
        .map(|character| if character.is_ascii_alphanumeric() { character } else { '-' })
        .collect::<String>();
    Some(ServiceRecord {
        id: format!("wsl-{}-container-{}", distribution_id, short_id),
        app_id: app_id.to_string(),
        name: app_name.to_string(),
        version: image_version(&container.config.image),
        target: if endpoint_published {
            format!(
                "WSL {} · {} · {}:{}",
                distribution,
                container.name.trim_start_matches('/'),
                host,
                port
            )
        } else {
            format!(
                "WSL {} · {} · 未发布主机端口",
                distribution,
                container.name.trim_start_matches('/')
            )
        },
        host,
        port,
        username: None,
        database: None,
        tls: "disabled".to_string(),
        management: management.to_string(),
        connection: connection.to_string(),
        runtime: runtime.to_string(),
        endpoint_published,
        health: "unverified".to_string(),
        credential_state: "missing".to_string(),
        capabilities,
        last_checked: Some(now_text()),
        task_id: None,
        progress: None,
        progress_label: None,
        distribution: Some(distribution.to_string()),
        compose_path,
        compose_project,
        container_id: Some(container.id),
    })
}

fn scan_wsl_environment() -> Result<WslScanResult, String> {
    let distributions = running_wsl_distributions()?;
    let mut panel_distributions = Vec::new();
    let mut running_panels = Vec::new();
    let mut stopped_panels = Vec::new();
    let mut services = Vec::new();
    let mut notices = Vec::new();

    for distribution in distributions {
        if let Some(running) = panel_status(&distribution) {
            panel_distributions.push(distribution.clone());
            if running {
                running_panels.push(distribution.clone());
            } else {
                stopped_panels.push(distribution.clone());
            }
        }
        match docker_inspects(&distribution) {
            Ok(containers) => {
                if containers.iter().any(is_panel_container)
                    && !panel_distributions.contains(&distribution)
                {
                    panel_distributions.push(distribution.clone());
                }
                services.extend(
                    containers.into_iter()
                        .filter_map(|container| service_from_container(&distribution, container)),
                );
            }
            Err(error) => notices.push(format!(
                "未能读取 WSL {} 的 Docker 容器：{}",
                distribution, error
            )),
        }
    }

    let panel_service_count = services.iter()
        .filter(|service| service.management == "1panel_compose")
        .count();
    let panel = if panel_distributions.is_empty() {
        PanelState {
            state: "not_found".to_string(),
            endpoint: None,
            message: "未在当前运行的 WSL 发行版中发现 1Panel。".to_string(),
        }
    } else {
        let state_text = if !running_panels.is_empty() {
            "服务正在运行"
        } else if stopped_panels.len() == panel_distributions.len() {
            "服务当前未运行"
        } else {
            "服务状态未确认"
        };
        PanelState {
            state: "read_only_discovered".to_string(),
            endpoint: None,
            message: format!(
                "已在 WSL {} 发现 1Panel，{}；依据容器标签和编排路径识别到 {} 个 1Panel 容器。当前为只读发现，1Panel API 管理暂未接入。",
                panel_distributions.join("、"),
                state_text,
                panel_service_count
            ),
        }
    };

    Ok(WslScanResult { panel, services, notices })
}

fn resolve_target(host: &str, port: u16) -> Result<SocketAddr, String> {
    if host.trim().is_empty() {
        return Err("主机地址不能为空。".to_string());
    }
    (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("域名或地址解析失败：{}", error))?
        .next()
        .ok_or_else(|| "没有解析到可用的目标地址。".to_string())
}

fn validate_connection_draft(draft: &ConnectionDraft) -> Result<(), String> {
    if !matches!(draft.app_id.as_str(), "mysql" | "redis" | "minio" | "qdrant" | "jenkins") {
        return Err("不支持的服务类型。".to_string());
    }
    if draft.name.trim().is_empty() {
        return Err("显示名称不能为空。".to_string());
    }
    let host = draft.host.trim();
    if host.is_empty() {
        return Err("主机地址不能为空。".to_string());
    }
    if host.contains("://") || host.chars().any(char::is_whitespace) {
        return Err("主机地址只能填写域名或 IP，不能包含协议或空白字符。".to_string());
    }
    if draft.port == 0 {
        return Err("端口必须在 1 到 65535 之间。".to_string());
    }
    if !matches!(draft.tls.as_str(), "disabled" | "required" | "custom_ca") {
        return Err("TLS 配置无效。".to_string());
    }
    let is_local = host.eq_ignore_ascii_case("localhost")
        || host == "::1"
        || host.starts_with("127.");
    if !is_local && !draft.remote_approved {
        return Err("目标不是本机地址，请先明确确认允许连接远程主机。".to_string());
    }
    if draft.password.as_deref().is_some_and(|value| !value.is_empty()) {
        return Err("当前构建尚未完成 Windows 凭据加密，已拒绝保存密码。".to_string());
    }
    Ok(())
}

fn probe_target(draft: &ConnectionDraft) -> Result<ConnectionProbeResult, String> {
    validate_connection_draft(draft)?;
    let address = resolve_target(draft.host.trim(), draft.port)?;
    match TcpStream::connect_timeout(&address, Duration::from_secs(4)) {
        Ok(_) => Ok(ConnectionProbeResult {
            state: "reachable".to_string(),
            summary: "TCP 端口可达；尚未执行协议鉴权，不能据此判断服务健康。".to_string(),
            checked_at: now_text(),
        }),
        Err(_) => Err("TCP 端口不可达，请检查地址、端口、防火墙和服务状态。".to_string()),
    }
}

fn package_manifest(package_id: &str) -> Result<PackageManifest, String> {
    embedded_package_manifests()
        .into_iter()
        .filter_map(|source| serde_json::from_str::<PackageManifest>(source).ok())
        .find(|manifest| manifest.id == package_id)
        .ok_or_else(|| "没有找到指定的内置应用包。".to_string())
}

fn deployment_distribution() -> Result<String, String> {
    let distributions = running_wsl_distributions()
        .map_err(|error| format!("读取正在运行的 WSL 发行版失败：{}", error))?;
    if distributions.is_empty() {
        return Err("没有正在运行的 WSL 发行版，请先启动安装了 Docker 的 WSL 环境。".to_string());
    }
    for distribution in distributions {
        if wsl_output(&distribution, &["docker", "info", "--format", "{{.ServerVersion}}"]).is_ok()
            && wsl_output(&distribution, &["docker", "compose", "version", "--short"]).is_ok()
        {
            return Ok(distribution);
        }
    }
    Err("未找到 Docker 服务和 Docker Compose 均可用的 WSL 发行版。".to_string())
}

fn available_loopback_port(preferred: u16) -> Result<u16, String> {
    (preferred..=preferred.saturating_add(100))
        .find(|port| TcpListener::bind(("127.0.0.1", *port)).is_ok())
        .ok_or_else(|| format!("本机 {} 到 {} 端口均被占用。", preferred, preferred.saturating_add(100)))
}

fn update_install_task(
    task_id: &str,
    service_id: &str,
    stage: &str,
    progress: u8,
    level: &str,
    message: &str,
) -> Result<bool, String> {
    let mut document = load_document()?;
    let task = document.snapshot.tasks.iter_mut()
        .find(|task| task.id == task_id)
        .ok_or_else(|| "没有找到 Jenkins 部署任务。".to_string())?;
    if task.status == "cancelled" {
        return Ok(false);
    }
    task.status = "running".to_string();
    task.stage = stage.to_string();
    task.progress = Some(progress);
    task.updated_at = now_text();
    task.events.push(TaskEvent {
        sequence: task.events.len() as u64 + 1,
        level: level.to_string(),
        message: message.to_string(),
        time: now_text(),
    });
    if let Some(service) = document.snapshot.services.iter_mut().find(|service| service.id == service_id) {
        service.progress = Some(progress);
        service.progress_label = Some(stage.to_string());
    }
    save_document(&document)?;
    Ok(true)
}

fn remove_cancelled_install(service_id: &str) -> Result<(), String> {
    let mut document = load_document()?;
    document.snapshot.services.retain(|service| service.id != service_id);
    save_document(&document)
}

fn fail_install_task(task_id: &str, service_id: &str, error: &str) {
    if let Ok(mut document) = load_document() {
        if document.snapshot.tasks.iter()
            .any(|task| task.id == task_id && task.status == "cancelled")
        {
            return;
        }
        if let Some(task) = document.snapshot.tasks.iter_mut().find(|task| task.id == task_id) {
            task.status = "failed".to_string();
            task.stage = "部署失败".to_string();
            task.updated_at = now_text();
            task.events.push(TaskEvent {
                sequence: task.events.len() as u64 + 1,
                level: "错误".to_string(),
                message: error.to_string(),
                time: now_text(),
            });
        }
        if let Some(service) = document.snapshot.services.iter_mut().find(|service| service.id == service_id) {
            service.runtime = "error".to_string();
            service.progress = None;
            service.progress_label = Some("部署失败".to_string());
        }
        document.snapshot.notices.push(error.to_string());
        let _ = save_document(&document);
    }
    let _ = append_diagnostic_log(error);
}

fn install_jenkins(
    task_id: String,
    service_id: String,
    distribution: String,
    port: u16,
) -> Result<(), String> {
    let manifest = package_manifest("jenkins")?;
    let repository = manifest.image_repository
        .ok_or_else(|| "Jenkins 包清单缺少镜像仓库。".to_string())?;
    let image_tag = manifest.image_tag
        .ok_or_else(|| "Jenkins 包清单缺少固定镜像标签。".to_string())?;
    let image_variable = manifest.image_variable
        .ok_or_else(|| "Jenkins 包清单缺少镜像变量。".to_string())?;
    if manifest.compose_template.as_deref() != Some("compose.yaml") {
        return Err("Jenkins 包清单中的 Compose 模板无效。".to_string());
    }
    let tagged_image = format!("{}:{}", repository, image_tag);

    if !update_install_task(
        &task_id,
        &service_id,
        "拉取 Jenkins 镜像",
        20,
        "信息",
        "正在从官方 Jenkins 镜像仓库拉取固定版本。",
    )? {
        remove_cancelled_install(&service_id)?;
        return Ok(());
    }
    wsl_output(&distribution, &["docker", "pull", &tagged_image])
        .map_err(|error| format!("拉取 Jenkins 镜像失败：{}", error))?;

    let digest_image = wsl_output(
        &distribution,
        &["docker", "image", "inspect", "--format", "{{index .RepoDigests 0}}", &tagged_image],
    ).map_err(|error| format!("读取 Jenkins 镜像摘要失败：{}", error))?;
    if !digest_image.starts_with(&format!("{}@sha256:", repository)) {
        return Err("Docker 未返回可核验的 Jenkins sha256 镜像摘要，已停止部署。".to_string());
    }
    if !update_install_task(
        &task_id,
        &service_id,
        "写入部署配置",
        58,
        "成功",
        "镜像已拉取，并已解析实际 sha256 摘要。",
    )? {
        remove_cancelled_install(&service_id)?;
        return Ok(());
    }

    let directory = data_dir()?.join("deployments").join("jenkins");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("创建 Jenkins 部署目录失败：{}", error))?;
    fs::write(
        directory.join("compose.yaml"),
        include_str!("../../app-packages/jenkins/compose.yaml"),
    ).map_err(|error| format!("写入 Jenkins Compose 配置失败：{}", error))?;
    fs::write(
        directory.join(".env"),
        format!("{}={}\nLOCALDOCK_JENKINS_PORT={}\n", image_variable, digest_image, port),
    ).map_err(|error| format!("写入 Jenkins 部署变量失败：{}", error))?;

    let windows_directory = directory.to_string_lossy().to_string();
    let linux_directory = wsl_output(
        &distribution,
        &["wslpath", "-a", "-u", &windows_directory],
    ).map_err(|error| format!("转换 Jenkins 部署目录失败：{}", error))?;
    let compose_file = format!("{}/compose.yaml", linux_directory);
    let env_file = format!("{}/.env", linux_directory);

    if !update_install_task(
        &task_id,
        &service_id,
        "启动 Jenkins 容器",
        76,
        "信息",
        "正在通过 Docker Compose 创建 Jenkins 容器和持久化数据卷。",
    )? {
        remove_cancelled_install(&service_id)?;
        return Ok(());
    }
    let compose_arguments = [
        "docker", "compose", "-p", "localdock-jenkins",
        "--project-directory", linux_directory.as_str(),
        "--env-file", env_file.as_str(),
        "-f", compose_file.as_str(),
        "up", "-d",
    ];
    wsl_output(&distribution, &compose_arguments)
        .map_err(|error| format!("启动 Jenkins Compose 项目失败：{}", error))?;

    if !update_install_task(
        &task_id,
        &service_id,
        "核验 Jenkins 容器",
        90,
        "信息",
        "容器已创建，正在读取实际容器、端口和运行状态。",
    )? {
        let mut down_arguments = compose_arguments;
        down_arguments[down_arguments.len() - 2] = "down";
        down_arguments[down_arguments.len() - 1] = "--remove-orphans";
        let _ = wsl_output(&distribution, &down_arguments);
        remove_cancelled_install(&service_id)?;
        return Ok(());
    }

    let container_id = wsl_output(
        &distribution,
        &[
            "docker", "compose", "-p", "localdock-jenkins",
            "--project-directory", linux_directory.as_str(),
            "--env-file", env_file.as_str(),
            "-f", compose_file.as_str(),
            "ps", "-q", "jenkins",
        ],
    ).map_err(|error| format!("读取 Jenkins 容器标识失败：{}", error))?;
    if container_id.trim().is_empty() {
        return Err("Jenkins Compose 项目没有返回容器标识。".to_string());
    }
    let inspect_output = wsl_output(&distribution, &["docker", "inspect", container_id.trim()])
        .map_err(|error| format!("读取 Jenkins 容器状态失败：{}", error))?;
    let mut containers: Vec<DockerInspect> = serde_json::from_str(&inspect_output)
        .map_err(|error| format!("解析 Jenkins 容器状态失败：{}", error))?;
    let mut service = containers.pop()
        .and_then(|container| service_from_container(&distribution, container))
        .ok_or_else(|| "Jenkins 容器已创建，但没有读取到可访问端口。".to_string())?;
    service.version = Some(manifest.version);
    service.task_id = Some(task_id.clone());

    let mut document = load_document()?;
    document.snapshot.services.retain(|item| item.id != service_id);
    document.snapshot.services.retain(|item| {
        !(item.app_id == "jenkins" && item.management == "localdock_compose")
    });
    let actual_service_id = service.id.clone();
    document.snapshot.services.push(service);
    let task = document.snapshot.tasks.iter_mut()
        .find(|task| task.id == task_id)
        .ok_or_else(|| "没有找到 Jenkins 部署任务。".to_string())?;
    task.instance_id = Some(actual_service_id);
    task.status = "completed".to_string();
    task.stage = "Jenkins 已启动".to_string();
    task.progress = Some(100);
    task.updated_at = now_text();
    task.events.push(TaskEvent {
        sequence: task.events.len() as u64 + 1,
        level: "成功".to_string(),
        message: format!("Jenkins 已在 127.0.0.1:{} 启动。", port),
        time: now_text(),
    });
    document.snapshot.scanned_at = now_text();
    document.snapshot.notices.push(format!("Jenkins 已通过独立 Compose 部署到本机端口 {}。", port));
    save_document(&document)?;
    append_diagnostic_log(&format!("Jenkins 部署完成，访问端口为 {}。", port))
}

#[tauri::command]
fn load_workspace() -> Result<WorkspaceSnapshot, String> {
    let document = load_document()?;
    if !asset_path()?.exists() {
        save_document(&document)?;
    }
    Ok(document.snapshot)
}

#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
fn check_for_updates() -> Result<UpdateCheckResult, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let script = format!(
        r#"$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$headers = @{{ 'User-Agent' = 'LocalDock/{}'; 'Accept' = 'application/vnd.github+json' }}
try {{
    $release = Invoke-RestMethod -UseBasicParsing -Headers $headers -Uri 'https://api.github.com/repos/joe194910/LocalDock/releases/latest' -TimeoutSec 15
    $release | Select-Object tag_name,name,html_url,published_at | ConvertTo-Json -Compress
}} catch {{
    [Console]::Error.Write($_.Exception.Message)
    exit 1
}}"#,
        current_version
    );
    let output = command_output(
        "powershell.exe",
        &[
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ],
    )
    .map_err(|error| format!("无法连接 GitHub 检查更新：{}", error))?;
    let release: GitHubRelease = serde_json::from_str(&output)
        .map_err(|error| format!("解析 GitHub 版本信息失败：{}", error))?;
    let latest_version = release
        .tag_name
        .trim()
        .trim_start_matches(['v', 'V'])
        .to_string();
    let update_available = is_newer_version(&latest_version, &current_version)?;
    Ok(UpdateCheckResult {
        current_version,
        latest_version: latest_version.clone(),
        update_available,
        release_name: release
            .name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| format!("LocalDock {}", latest_version)),
        release_url: release.html_url,
        published_at: release.published_at,
    })
}

#[tauri::command]
fn open_releases_page() -> Result<(), String> {
    background_command("explorer.exe")
        .arg("https://github.com/joe194910/LocalDock/releases")
        .spawn()
        .map_err(|error| format!("打开 GitHub 发布页面失败：{}", error))?;
    Ok(())
}

#[tauri::command]
fn start_package_install(package_id: String) -> Result<WorkspaceSnapshot, String> {
    if package_id != "jenkins" {
        return Err("当前版本只开放了 Jenkins 的真实独立 Compose 安装流程。".to_string());
    }
    let distribution = deployment_distribution()?;
    let port = available_loopback_port(8080)?;
    let mut document = load_document()?;
    if document.settings.default_deployment_backend != "compose" {
        return Err("Jenkins 当前只能通过独立 Compose 安装，请先在设置中选择“独立 Compose”。".to_string());
    }
    if document.snapshot.tasks.iter().any(|task| {
        task.title.contains("Jenkins") && matches!(task.status.as_str(), "queued" | "running")
    }) {
        return Err("已经有一个 Jenkins 部署任务正在执行。".to_string());
    }
    if document.snapshot.services.iter().any(|service| {
        service.app_id == "jenkins"
            && service.management == "localdock_compose"
            && matches!(service.runtime.as_str(), "running" | "installing")
    }) {
        return Err("LocalDock 管理的 Jenkins 已存在，请先查看现有服务。".to_string());
    }

    let suffix = unix_time();
    let task_id = format!("deploy-jenkins-{}", suffix);
    let service_id = format!("installing-jenkins-{}", suffix);
    document.snapshot.tasks.insert(0, DeploymentTask {
        id: task_id.clone(),
        instance_id: Some(service_id.clone()),
        title: "安装 Jenkins LTS".to_string(),
        stage: "等待后台部署".to_string(),
        status: "queued".to_string(),
        progress: Some(5),
        transferred: None,
        total: None,
        updated_at: now_text(),
        events: vec![TaskEvent {
            sequence: 1,
            level: "信息".to_string(),
            message: format!("已选择 WSL {}，计划使用本机端口 {}。", distribution, port),
            time: now_text(),
        }],
    });
    document.snapshot.services.push(ServiceRecord {
        id: service_id.clone(),
        app_id: "jenkins".to_string(),
        name: "Jenkins".to_string(),
        version: Some("2.479.3".to_string()),
        target: format!("WSL {} · 127.0.0.1:{}", distribution, port),
        host: "127.0.0.1".to_string(),
        port,
        username: None,
        database: None,
        tls: "disabled".to_string(),
        management: "localdock_compose".to_string(),
        connection: "unknown".to_string(),
        runtime: "installing".to_string(),
        endpoint_published: true,
        health: "unverified".to_string(),
        credential_state: "missing".to_string(),
        capabilities: Vec::new(),
        last_checked: Some(now_text()),
        task_id: Some(task_id.clone()),
        progress: Some(5),
        progress_label: Some("等待后台部署".to_string()),
        distribution: Some(distribution.clone()),
        compose_path: None,
        compose_project: Some("localdock-jenkins".to_string()),
        container_id: None,
    });
    save_document(&document)?;
    let snapshot = document.snapshot.clone();
    std::thread::spawn(move || {
        if let Err(error) = install_jenkins(task_id.clone(), service_id.clone(), distribution, port) {
            fail_install_task(&task_id, &service_id, &error);
        }
    });
    Ok(snapshot)
}

#[tauri::command]
fn get_app_settings() -> Result<AppSettings, String> {
    Ok(load_document()?.settings)
}

#[tauri::command]
fn set_default_deployment_backend(backend: String) -> Result<AppSettings, String> {
    if !matches!(backend.as_str(), "compose" | "panel") {
        return Err("不支持的默认部署方式。".to_string());
    }
    let mut document = load_document()?;
    document.settings.default_deployment_backend = backend;
    save_document(&document)?;
    let _ = append_diagnostic_log("默认部署方式已更新。");
    Ok(document.settings)
}

#[tauri::command]
fn get_storage_locations() -> Result<StorageLocations, String> {
    let config_file = asset_path()?;
    let log_file = diagnostic_log_path()?;
    if let Some(parent) = log_file.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建诊断日志目录失败：{}", error))?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .map_err(|error| format!("创建诊断日志文件失败：{}", error))?;
    Ok(StorageLocations {
        config_file: config_file.to_string_lossy().to_string(),
        log_file: log_file.to_string_lossy().to_string(),
    })
}

#[tauri::command]
fn open_storage_directory(kind: String) -> Result<(), String> {
    let file_path = match kind.as_str() {
        "config" => asset_path()?,
        "log" => diagnostic_log_path()?,
        _ => return Err("不支持的本地目录类型。".to_string()),
    };
    let directory = file_path.parent()
        .ok_or_else(|| "无法确定文件所在目录。".to_string())?;
    background_command("explorer.exe")
        .arg(directory)
        .spawn()
        .map_err(|error| format!("打开本地目录失败：{}", error))?;
    Ok(())
}

#[tauri::command]
fn scan_local_machine() -> Result<WorkspaceSnapshot, String> {
    let mut document = load_document()?;
    document.snapshot.tools = scan_tools();
    match scan_wsl_environment() {
        Ok(result) => {
            document.snapshot.services.retain(|service| {
                !matches!(service.management.as_str(), "1panel_compose" | "external_container")
                    && !(service.management == "localdock_compose" && service.runtime != "installing")
            });
            document.snapshot.services.extend(result.services);
            document.snapshot.panel = result.panel;
            document.snapshot.notices.extend(result.notices);
        }
        Err(error) => {
            document.snapshot.panel = PanelState {
                state: "error".to_string(),
                endpoint: None,
                message: format!("扫描 WSL 与 1Panel 失败：{}", error),
            };
            document.snapshot.notices.push(
                "本次扫描失败，已保留上一次发现的容器记录，未伪造空结果。".to_string(),
            );
        }
    }
    document.snapshot.scanned_at = now_text();
    document.snapshot.mode = "real".to_string();
    save_document(&document)?;
    let _ = append_diagnostic_log("本机环境扫描完成，工具、WSL 和容器状态已更新。");
    Ok(document.snapshot)
}

#[tauri::command]
fn test_connection(service_id: String) -> Result<WorkspaceSnapshot, String> {
    let mut document = load_document()?;
    let service = document.snapshot.services.iter_mut()
        .find(|item| item.id == service_id)
        .ok_or_else(|| "没有找到指定的服务记录。".to_string())?;
    if !service.capabilities.iter().any(|item| item == "test") {
        return Err("当前服务记录不允许执行连接测试。".to_string());
    }
    let address = resolve_target(&service.host, service.port)?;
    match TcpStream::connect_timeout(&address, Duration::from_secs(4)) {
        Ok(_) => {
            service.connection = "reachable".to_string();
            service.health = "unverified".to_string();
            service.last_checked = Some(now_text());
            document.snapshot.notices.push(
                "端口可达，但当前版本尚未完成协议鉴权，不能报告为已验证健康。".to_string(),
            );
            save_document(&document)?;
            Ok(document.snapshot)
        }
        Err(error) => {
            service.connection = "failed".to_string();
            service.last_checked = Some(now_text());
            save_document(&document)?;
            Err(format!("连接目标失败：{}", error))
        }
    }
}

#[tauri::command]
fn probe_connection(draft: ConnectionDraft) -> Result<ConnectionProbeResult, String> {
    probe_target(&draft)
}

#[tauri::command]
fn save_external_connection(draft: ConnectionDraft) -> Result<WorkspaceSnapshot, String> {
    validate_connection_draft(&draft)?;
    let checked_at = now_text();
    let probe = probe_target(&draft);
    if probe.is_err() && !draft.save_unverified {
        return Err("端口探测失败，且未允许保存未验证连接。".to_string());
    }

    let mut document = load_document()?;
    let connection = if probe.is_ok() { "reachable" } else { "unknown" };
    let target = format!("{}:{}", draft.host.trim(), draft.port);
    let mut capabilities = vec!["test".to_string(), "copy_connection".to_string()];
    if matches!(draft.app_id.as_str(), "minio" | "qdrant" | "jenkins") {
        capabilities.push("open_browser".to_string());
    }
    let service = ServiceRecord {
        id: format!("external-{}-{}", draft.app_id, unix_time()),
        app_id: draft.app_id,
        name: draft.name.trim().to_string(),
        version: None,
        target,
        host: draft.host.trim().to_string(),
        port: draft.port,
        username: draft.username.filter(|value| !value.trim().is_empty()),
        database: draft.database.filter(|value| !value.trim().is_empty()),
        tls: draft.tls,
        management: "external_connection".to_string(),
        connection: connection.to_string(),
        runtime: "unknown".to_string(),
        endpoint_published: true,
        health: "unverified".to_string(),
        credential_state: "missing".to_string(),
        capabilities,
        last_checked: Some(checked_at),
        task_id: None,
        progress: None,
        progress_label: None,
        distribution: None,
        compose_path: None,
        compose_project: None,
        container_id: None,
    };
    document.snapshot.services.push(service);
    document.snapshot.notices.push(if probe.is_ok() {
        "已保存外部连接；TCP 端口可达，但协议鉴权仍未完成。".to_string()
    } else {
        "已按用户选择保存未验证连接；未把失败探测报告为成功。".to_string()
    });
    save_document(&document)?;
    Ok(document.snapshot)
}

#[tauri::command]
fn open_service_endpoint(service_id: String) -> Result<(), String> {
    let document = load_document()?;
    let service = document.snapshot.services.iter()
        .find(|item| item.id == service_id)
        .ok_or_else(|| "没有找到指定的服务记录。".to_string())?;
    if !service.capabilities.iter().any(|item| item == "open_browser") {
        return Err("该服务不具备浏览器入口，已拒绝打开。".to_string());
    }
    if !matches!(service.app_id.as_str(), "minio" | "qdrant" | "jenkins") {
        return Err("该服务使用非网页协议，请复制连接信息。".to_string());
    }
    let scheme = if service.tls == "disabled" { "http" } else { "https" };
    let url = format!("{}://{}:{}", scheme, service.host, service.port);
    Command::new("explorer.exe")
        .arg(&url)
        .spawn()
        .map_err(|error| format!("调用系统浏览器失败：{}", error))?;
    Ok(())
}

#[tauri::command]
fn get_jenkins_initial_password(service_id: String) -> Result<String, String> {
    let document = load_document()?;
    let service = document.snapshot.services.iter()
        .find(|service| service.id == service_id)
        .ok_or_else(|| "没有找到指定的 Jenkins 服务。".to_string())?;
    if service.app_id != "jenkins" || service.management != "localdock_compose" {
        return Err("只有 LocalDock 管理的 Jenkins 才能读取初始管理员密码。".to_string());
    }
    let distribution = service.distribution.as_deref()
        .ok_or_else(|| "Jenkins 记录缺少 WSL 发行版信息。".to_string())?;
    let container_id = service.container_id.as_deref()
        .ok_or_else(|| "Jenkins 记录缺少实际容器标识。".to_string())?;
    let password = wsl_output(
        distribution,
        &[
            "docker", "exec", container_id,
            "cat", "/var/jenkins_home/secrets/initialAdminPassword",
        ],
    ).map_err(|error| format!("读取 Jenkins 初始管理员密码失败：{}", error))?;
    if password.trim().is_empty() {
        return Err("Jenkins 尚未生成初始管理员密码，请稍后重试。".to_string());
    }
    Ok(password.trim().to_string())
}

#[tauri::command]
fn cancel_task(task_id: String) -> Result<WorkspaceSnapshot, String> {
    let mut document = load_document()?;
    let task = document.snapshot.tasks.iter_mut()
        .find(|item| item.id == task_id)
        .ok_or_else(|| "没有找到指定任务。".to_string())?;
    if task.status != "running" && task.status != "queued" {
        return Err("只有等待中或运行中的任务可以取消。".to_string());
    }
    task.status = "cancelled".to_string();
    task.stage = "用户已请求取消".to_string();
    task.updated_at = now_text();
    task.events.push(TaskEvent {
        sequence: task.events.len() as u64 + 1,
        level: "警告".to_string(),
        message: "任务已标记为取消；恢复前会先观察实际资源状态。".to_string(),
        time: now_text(),
    });
    let instance_id = task.instance_id.clone();
    if let Some(instance_id) = instance_id {
        document.snapshot.services.retain(|service| {
            service.id != instance_id || service.runtime != "installing"
        });
    }
    save_document(&document)?;
    Ok(document.snapshot)
}

#[tauri::command]
fn run_service_action(service_id: String, action: String) -> Result<WorkspaceSnapshot, String> {
    let mut document = load_document()?;
    let service = document.snapshot.services.iter()
        .find(|item| item.id == service_id)
        .cloned()
        .ok_or_else(|| "没有找到指定的服务记录。".to_string())?;
    let backend: Box<dyn ContainerBackend> = match service.management.as_str() {
        "localdock_compose" => Box::new(ComposeBackend),
        "1panel_compose" => Box::new(PanelBackend),
        _ => return Err("该记录不是 LocalDock 可写管理的实例。".to_string()),
    };
    if backend.kind() != service.management {
        return Err("实例管理归属与后端不一致，已拒绝执行。".to_string());
    }
    backend.run_action(&service, &action)?;
    if let Some(record) = document.snapshot.services.iter_mut().find(|item| item.id == service_id) {
        record.runtime = if action == "stop" { "stopped" } else { "running" }.to_string();
        record.last_checked = Some(now_text());
    }
    save_document(&document)?;
    Ok(document.snapshot)
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            load_workspace,
            get_app_version,
            check_for_updates,
            open_releases_page,
            start_package_install,
            get_app_settings,
            set_default_deployment_backend,
            get_storage_locations,
            open_storage_directory,
            scan_local_machine,
            test_connection,
            probe_connection,
            save_external_connection,
            open_service_endpoint,
            get_jenkins_initial_password,
            cancel_task,
            run_service_action
        ])
        .run(tauri::generate_context!())
        .expect("启动 LocalDock 桌面应用失败");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 可解析_wsl_的_utf16_输出() {
        let source = "Ubuntu-24.04\r\n自定义发行版\r\n";
        let bytes = source
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(decode_process_output(&bytes), source);
    }

    #[test]
    fn 容器识别不依赖固定版本和固定端口() {
        let input = r#"[
            {
                "Id": "1234567890abcdef",
                "Name": "/customer-redis",
                "State": { "Status": "running" },
                "Config": {
                    "Image": "registry.example.com/team/redis:9.2-custom",
                    "Labels": {
                        "com.docker.compose.project": "cache-stack",
                        "com.docker.compose.service": "redis",
                        "com.docker.compose.project.working_dir": "/srv/panel-data/1panel/apps/cache"
                    }
                },
                "NetworkSettings": {
                    "Ports": {
                        "6379/tcp": [
                            { "HostIp": "0.0.0.0", "HostPort": "16379" }
                        ]
                    }
                }
            }
        ]"#;
        let mut containers: Vec<DockerInspect> = serde_json::from_str(input).unwrap();
        let service = service_from_container("My-WSL", containers.remove(0)).unwrap();
        assert_eq!(service.app_id, "redis");
        assert_eq!(service.version.as_deref(), Some("9.2-custom"));
        assert_eq!(service.port, 16379);
        assert_eq!(service.management, "1panel_compose");
        assert!(service.id.starts_with("wsl-My-WSL-container-"));
    }

    #[test]
    fn 已停止且没有主机端口的已知容器仍会被发现() {
        let input = r#"[
            {
                "Id": "mysqlstopped123456",
                "Name": "/customer-mysql",
                "State": { "Status": "exited" },
                "Config": {
                    "Image": "mysql:8.0.46",
                    "Labels": null
                },
                "NetworkSettings": {
                    "Ports": {
                        "3306/tcp": null
                    }
                }
            }
        ]"#;
        let mut containers: Vec<DockerInspect> = serde_json::from_str(input).unwrap();
        let service = service_from_container("Ubuntu", containers.remove(0)).unwrap();

        assert_eq!(service.app_id, "mysql");
        assert_eq!(service.runtime, "stopped");
        assert!(!service.endpoint_published);
        assert_eq!(service.port, 3306);
        assert!(service.target.contains("未发布主机端口"));
        assert!(!service.capabilities.iter().any(|capability| capability == "test"));
    }

    #[test]
    fn 已暂停容器会保留暂停状态() {
        let input = r#"[
            {
                "Id": "redispaused123456",
                "Name": "/customer-redis",
                "State": { "Status": "paused" },
                "Config": {
                    "Image": "redis:7.4",
                    "Labels": null
                },
                "NetworkSettings": {
                    "Ports": {
                        "6379/tcp": [
                            { "HostIp": "127.0.0.1", "HostPort": "6379" }
                        ]
                    }
                }
            }
        ]"#;
        let mut containers: Vec<DockerInspect> = serde_json::from_str(input).unwrap();
        let service = service_from_container("Ubuntu", containers.remove(0)).unwrap();

        assert_eq!(service.runtime, "paused");
        assert!(service.endpoint_published);
    }

    #[test]
    fn 空标签和空端口会被安全跳过() {
        let input = r#"[
            {
                "Id": "abcdef1234567890",
                "Name": "/unknown",
                "State": { "Status": "exited" },
                "Config": {
                    "Image": "example/unknown:latest",
                    "Labels": null
                },
                "NetworkSettings": {
                    "Ports": null
                }
            }
        ]"#;
        let mut containers: Vec<DockerInspect> = serde_json::from_str(input).unwrap();
        assert!(service_from_container("任意发行版", containers.remove(0)).is_none());
    }

    #[test]
    fn 旧版资产缺少设置时使用独立编排默认值() {
        let document = AssetDocument {
            schema_version: 2,
            snapshot: empty_snapshot(),
            settings: AppSettings::default(),
        };
        let mut value = serde_json::to_value(document).unwrap();
        value.as_object_mut().unwrap().remove("settings");
        let migrated: AssetDocument = serde_json::from_value(value).unwrap();
        assert_eq!(migrated.settings.default_deployment_backend, "compose");
    }

    #[test]
    fn 内置目录来自真实包清单且标识唯一() {
        let packages = default_packages();
        let mut ids = packages.iter()
            .map(|package| package.id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();

        assert_eq!(packages.len(), 9);
        assert_eq!(ids.len(), packages.len());
        assert!(packages.iter().all(|package| package.trusted));
        assert!(packages.iter().any(|package| package.id == "temurin-jdk"));
        assert!(packages.iter().any(|package| package.id == "redis"));
    }

    #[test]
    fn jenkins_清单声明官方固定镜像和_compose_模板() {
        let manifest = package_manifest("jenkins").unwrap();
        assert_eq!(manifest.image_repository.as_deref(), Some("jenkins/jenkins"));
        assert_eq!(manifest.image_tag.as_deref(), Some("2.479.3-lts-jdk17"));
        assert_eq!(manifest.compose_template.as_deref(), Some("compose.yaml"));
        assert_ne!(manifest.image_tag.as_deref(), Some("latest"));
    }

    #[test]
    fn localdock_容器可恢复真实管理信息() {
        let input = r#"[
            {
                "Id": "abcdef1234567890",
                "Name": "/localdock-jenkins-1",
                "State": { "Status": "running" },
                "Config": {
                    "Image": "jenkins/jenkins@sha256:1234",
                    "Labels": {
                        "cn.localdock.managed": "true",
                        "cn.localdock.package": "jenkins",
                        "com.docker.compose.project": "localdock-jenkins",
                        "com.docker.compose.service": "jenkins",
                        "com.docker.compose.project.working_dir": "/mnt/c/Users/test/AppData/Local/LocalDock/deployments/jenkins"
                    }
                },
                "NetworkSettings": {
                    "Ports": {
                        "8080/tcp": [
                            { "HostIp": "127.0.0.1", "HostPort": "8088" }
                        ]
                    }
                }
            }
        ]"#;
        let mut containers: Vec<DockerInspect> = serde_json::from_str(input).unwrap();
        let service = service_from_container("Ubuntu", containers.remove(0)).unwrap();
        assert_eq!(service.management, "localdock_compose");
        assert_eq!(service.container_id.as_deref(), Some("abcdef1234567890"));
        assert_eq!(service.compose_project.as_deref(), Some("localdock-jenkins"));
        assert!(service.capabilities.iter().any(|capability| capability == "restart"));
    }

    #[test]
    fn 版本比较可以识别新版本() {
        assert!(is_newer_version("0.1.8", "0.1.7").unwrap());
        assert!(!is_newer_version("0.1.7", "0.1.8").unwrap());
    }

    #[test]
    fn 版本比较支持_v_前缀和不同长度() {
        assert!(is_newer_version("v1.2.1", "1.2").unwrap());
        assert!(!is_newer_version("V1.2.0", "1.2").unwrap());
    }

    #[test]
    fn 非法版本号会返回中文错误() {
        let error = is_newer_version("latest", "0.1.7").unwrap_err();
        assert!(error.contains("版本号"));
        assert!(error.contains("格式无效"));
    }
}
