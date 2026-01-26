use anyhow::Result;
use chrono::{DateTime, FixedOffset, Local, SecondsFormat, Utc};
use log::{debug, error, info, warn};
use rumqttc::{Client, Event, MqttOptions, Packet, QoS, TlsConfiguration, Transport};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime};
use supabase_realtime_rs::{
    PostgresChangeEvent, PostgresChangesFilter, RealtimeClient, RealtimeClientOptions,
};
use tauri::async_runtime::{self, JoinHandle};
use tauri::{Emitter, WindowEvent};

static ALERT_STORE: OnceLock<Mutex<HashMap<String, Alert>>> = OnceLock::new();
const ALERT_ADDED_EVENT: &str = "alerts://added";
const ALERT_REMOVED_EVENT: &str = "alerts://removed";
static BUZZER_CONTROLLER: OnceLock<Mutex<BuzzerController>> = OnceLock::new();
static MUTE_CONTROLLER: OnceLock<Mutex<MuteController>> = OnceLock::new();
const MUTE_CHANGED_EVENT: &str = "alerts://mute_changed";
static APP_CONFIG: OnceLock<AppConfig> = OnceLock::new();
static LOGGER_INITIALIZED: OnceLock<()> = OnceLock::new();
const CONFIG_PATH: &str = "config/config.yaml";

static MQTT_CONNECTED: AtomicBool = AtomicBool::new(false);
const MQTT_RETRY_DELAY: Duration = Duration::from_secs(5);
const MQTT_MAX_RETRY_DELAY: Duration = Duration::from_secs(60);
pub const MQTT_RPC_REQUEST_TOPIC: &str = "v1/devices/me/rpc/request/+";

static SUPABASE_CONNECTED: AtomicBool = AtomicBool::new(false);
const SUPABASE_RETRY_DELAY: Duration = Duration::from_secs(5);
const SUPABASE_MAX_RETRY_DELAY: Duration = Duration::from_secs(60);
const SUPABASE_CHANNEL_NAME: &str = "schema-db-changes";
const SUPABASE_DB_SCHEMA: &str = "public";
const BINARY_ARRAY_SIZE: usize = 6;
const DEVICE_STATUS_EVENT: &str = "device://status_changed";

static SHUTDOWN: AtomicBool = AtomicBool::new(false);
const BUZZER_FAILURE_LIMIT: u8 = 5;
const SLEEP_CHUNK: Duration = Duration::from_millis(200);
static BUZZER_GPIO_CACHE: OnceLock<Mutex<Option<(String, String)>>> = OnceLock::new();

const REFRIGERATOR_NAMES: [&str; 6] = [
    "Bodega - microbiología refri 2",
    "Bodega - microbiología refri 1",
    "Bodega - química refri 1",
    "Bodega - banco de sangre",
    "Bodega - química refri 2",
    "Bodega - Inmunología refri 1",
];
const TEMPERATURE_ALARM_TYPE: &str = "Temperature out of range";
const TEMPERATURE_ALARM_DESCRIPTION: &str = "Temperatura fuera de rango 2 - 8 °C";
static REFRIGERATOR_ALARM_STATE: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct AppConfig {
    mqtt_server: String,
    mqtt_use_secure_client: bool,
    mqtt_port: u16,
    mqtt_client_id: String,
    mqtt_username: String,
    mqtt_password: String,
    mute_duration: u64,
    #[serde(default = "default_buzzer_enabled")]
    buzzer_enabled: bool,
    #[serde(default)]
    supabase_url: String,
    #[serde(default)]
    supabase_anon_key: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mqtt_server: "j0661b06.ala.us-east-1.emqxsl.com".to_string(),
            mqtt_use_secure_client: true,
            mqtt_port: 8883,
            mqtt_client_id: "hmi-cli".to_string(),
            mqtt_username: "test".to_string(),
            mqtt_password: "test".to_string(),
            mute_duration: 600,
            buzzer_enabled: default_buzzer_enabled(),
            supabase_url: String::new(),
            supabase_anon_key: String::new(),
        }
    }
}

fn default_buzzer_enabled() -> bool {
    true
}

fn init_logging() {
    LOGGER_INITIALIZED.get_or_init(|| {
        let env = env_logger::Env::default().default_filter_or("info");
        if let Err(err) = env_logger::Builder::from_env(env)
            .format(|buf, record| {
                writeln!(
                    buf,
                    "[{}][{}] {}",
                    buf.timestamp_millis(),
                    record.level(),
                    record.args()
                )
            })
            .try_init()
        {
            eprintln!("[LOG] No se pudo inicializar logger: {:?}", err);
        }
    });
}

fn app_config() -> &'static AppConfig {
    APP_CONFIG.get_or_init(load_or_create_config)
}

fn load_or_create_config() -> AppConfig {
    let path = Path::new(CONFIG_PATH);
    match fs::read_to_string(path) {
        Ok(contents) if !contents.trim().is_empty() => match serde_yaml::from_str(&contents) {
            Ok(cfg) => cfg,
            Err(err) => {
                error!("[CONFIG] Error al parsear {}: {:?}", CONFIG_PATH, err);
                persist_default_config(path)
            }
        },
        _ => persist_default_config(path),
    }
}

fn persist_default_config(path: &Path) -> AppConfig {
    let default_cfg = AppConfig::default();
    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            error!("[CONFIG] No se pudo crear carpeta {:?}: {:?}", parent, err);
            return default_cfg;
        }
    }

    match serde_yaml::to_string(&default_cfg) {
        Ok(yaml) => {
            if let Err(err) = fs::write(path, yaml) {
                error!("[CONFIG] No se pudo escribir {:?}: {:?}", path, err);
            }
        }
        Err(err) => error!("[CONFIG] No se pudo serializar config por defecto: {:?}", err),
    }

    default_cfg
}

fn mute_duration() -> Duration {
    Duration::from_secs(app_config().mute_duration.max(1))
}

fn is_buzzer_enabled() -> bool {
    app_config().buzzer_enabled
}

fn buzzer_gpio_cache() -> &'static Mutex<Option<(String, String)>> {
    BUZZER_GPIO_CACHE.get_or_init(|| Mutex::new(None))
}

fn is_shutting_down() -> bool {
    SHUTDOWN.load(Ordering::SeqCst)
}

fn next_retry_delay(current: Duration) -> Duration {
    (current * 2).min(MQTT_MAX_RETRY_DELAY)
}

fn sleep_with_shutdown(total: Duration) {
    let mut elapsed = Duration::ZERO;
    while elapsed < total {
        if is_shutting_down() {
            break;
        }
        let remaining = total.saturating_sub(elapsed);
        let slice = if remaining < SLEEP_CHUNK {
            remaining
        } else {
            SLEEP_CHUNK
        };
        if slice.is_zero() {
            break;
        }
        thread::sleep(slice);
        elapsed = elapsed.saturating_add(slice);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AlertType {
    #[serde(rename = "disconnect")]
    Disconnect,
    #[serde(rename = "tempUp")]
    TempUp,
    #[serde(rename = "tempDown")]
    TempDown,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Alert {
    pub id: String,

    #[serde(rename = "dateTime")]
    pub date_time: String,

    #[serde(rename = "type")]
    pub alert_type: AlertType,

    pub device: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
struct AlarmRpcEnvelope {
    method: String,
    params: AlarmParams,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AlarmParams {
    id: AlarmEntityId,
    created_time: i64,
    #[serde(rename = "type")]
    alarm_type: String,
    originator_name: String,
    status: AlarmStatus,
    #[serde(default)]
    details: Option<AlarmDetails>,
}

#[derive(Debug, Deserialize, Clone)]
struct AlarmEntityId {
    #[serde(rename = "id")]
    value: String,
}

#[derive(Debug, Deserialize, Clone)]
struct AlarmDetails {
    #[serde(default)]
    data: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum AlarmStatus {
    ActiveUnack,
    ClearedUnack,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct SupabaseUpdatePayload {
    commit_timestamp: String,
    new: SupabaseNewData,
}

#[derive(Debug, Deserialize)]
struct SupabaseNewData {
    message: String,
}

#[derive(Debug, Serialize, Clone)]
struct DeviceStatusUpdate {
    timestamp: String,
    status: Vec<u8>,
}

#[derive(Debug, Serialize)]
struct AlertRemovalEvent {
    id: String,
}

struct BuzzerController {
    handle: Option<JoinHandle<()>>,
}

impl Default for BuzzerController {
    fn default() -> Self {
        Self { handle: None }
    }
}

struct MuteController {
    muted: bool,
    deadline: Option<SystemTime>,
    timer: Option<JoinHandle<()>>,
}

impl Default for MuteController {
    fn default() -> Self {
        Self {
            muted: false,
            deadline: None,
            timer: None,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
struct MuteStatePayload {
    muted: bool,
    #[serde(rename = "expiresAt")]
    expires_at: Option<String>,
}

fn with_alert_store<F, R>(f: F) -> R
where
    F: FnOnce(&mut HashMap<String, Alert>) -> R,
{
    let store = ALERT_STORE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = store
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

fn with_buzzer_controller<F, R>(f: F) -> R
where
    F: FnOnce(&mut BuzzerController) -> R,
{
    let controller = BUZZER_CONTROLLER.get_or_init(|| Mutex::new(BuzzerController::default()));
    let mut guard = controller
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

fn with_mute_controller<F, R>(f: F) -> R
where
    F: FnOnce(&mut MuteController) -> R,
{
    let controller = MUTE_CONTROLLER.get_or_init(|| Mutex::new(MuteController::default()));
    let mut guard = controller
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

fn snapshot_mute_state() -> MuteStatePayload {
    with_mute_controller(|ctrl| MuteStatePayload {
        muted: ctrl.muted,
        expires_at: format_deadline(ctrl.deadline),
    })
}

fn format_deadline(deadline: Option<SystemTime>) -> Option<String> {
    deadline.map(|ts| {
        let datetime: chrono::DateTime<Utc> = ts.into();
        datetime.to_rfc3339_opts(SecondsFormat::Secs, true)
    })
}

fn emit_mute_state(app_handle: &tauri::AppHandle, payload: &MuteStatePayload) {
    if let Err(err) = app_handle.emit(MUTE_CHANGED_EVENT, payload) {
        warn!("[MUTE] No se pudo emitir estado mute: {:?}", err);
    }
}

fn cancel_mute_timer(ctrl: &mut MuteController) {
    if let Some(handle) = ctrl.timer.take() {
        handle.abort();
    }
}

fn schedule_mute_timer(app_handle: &tauri::AppHandle) -> JoinHandle<()> {
    let app_handle = app_handle.clone();
    async_runtime::spawn(async move {
        tokio::time::sleep(mute_duration()).await;
        handle_mute_timeout(app_handle);
    })
}

fn handle_mute_timeout(app_handle: tauri::AppHandle) {
    let should_emit = with_mute_controller(|ctrl| {
        if ctrl.muted {
            ctrl.muted = false;
            ctrl.deadline = None;
            ctrl.timer = None;
            true
        } else {
            ctrl.timer = None;
            false
        }
    });

    if !should_emit {
        return;
    }

    if has_active_alerts() {
        set_buzzer_state(true);
    } else {
        set_buzzer_state(false);
    }

    let payload = snapshot_mute_state();
    emit_mute_state(&app_handle, &payload);
}

fn has_active_alerts() -> bool {
    with_alert_store(|store| !store.is_empty())
}

fn force_unmute(app_handle: &tauri::AppHandle) -> Option<MuteStatePayload> {
    let changed = with_mute_controller(|ctrl| {
        if ctrl.muted || ctrl.deadline.is_some() || ctrl.timer.is_some() {
            ctrl.muted = false;
            ctrl.deadline = None;
            cancel_mute_timer(ctrl);
            true
        } else {
            false
        }
    });

    if changed {
        let payload = snapshot_mute_state();
        emit_mute_state(app_handle, &payload);
        Some(payload)
    } else {
        None
    }
}

fn mute_alerts_internal(app_handle: &tauri::AppHandle) -> MuteStatePayload {
    let expires_at = SystemTime::now()
        .checked_add(mute_duration())
        .unwrap_or_else(|| SystemTime::now());
    let timer = schedule_mute_timer(app_handle);

    with_mute_controller(|ctrl| {
        cancel_mute_timer(ctrl);
        ctrl.muted = true;
        ctrl.deadline = Some(expires_at);
        ctrl.timer = Some(timer);
    });

    set_buzzer_state(false);

    let payload = snapshot_mute_state();
    emit_mute_state(app_handle, &payload);
    payload
}

fn handle_alert_activation_side_effects(app_handle: &tauri::AppHandle) {
    let mut unmuted = false;
    with_mute_controller(|ctrl| {
        if ctrl.muted {
            ctrl.muted = false;
            ctrl.deadline = None;
            cancel_mute_timer(ctrl);
            unmuted = true;
        }
    });

    if unmuted {
        let payload = snapshot_mute_state();
        emit_mute_state(app_handle, &payload);
    }

    set_buzzer_state(true);
}

fn handle_no_active_alerts(app_handle: &tauri::AppHandle) {
    let mut changed = false;
    with_mute_controller(|ctrl| {
        if ctrl.muted || ctrl.deadline.is_some() || ctrl.timer.is_some() {
            ctrl.muted = false;
            ctrl.deadline = None;
            cancel_mute_timer(ctrl);
            changed = true;
        }
    });

    if changed {
        let payload = snapshot_mute_state();
        emit_mute_state(app_handle, &payload);
    }

    set_buzzer_state(false);
}

fn snapshot_alerts() -> Vec<Alert> {
    with_alert_store(|store| store.values().cloned().collect())
}

fn validate_binary_array(message: &str) -> Result<Vec<u8>> {
    let values: Vec<u8> = serde_json::from_str(message)
        .map_err(|e| anyhow::anyhow!("Formato JSON inválido: {}", e))?;

    if values.len() != BINARY_ARRAY_SIZE {
        return Err(anyhow::anyhow!(
            "El array debe tener exactamente {} elementos, pero tiene {}",
            BINARY_ARRAY_SIZE,
            values.len()
        ));
    }

    for (index, &value) in values.iter().enumerate() {
        if value != 0 && value != 1 {
            return Err(anyhow::anyhow!(
                "El elemento en posición {} tiene valor {}, debe ser 0 o 1",
                index, value
            ));
        }
    }

    Ok(values)
}

fn parse_supabase_timestamp(timestamp: &str) -> String {
    let guatemala_tz = FixedOffset::west_opt(6 * 3600).unwrap_or_else(|| FixedOffset::west_opt(0).unwrap());
    
    match timestamp.parse::<DateTime<Utc>>() {
        Ok(utc_time) => utc_time
            .with_timezone(&guatemala_tz)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
        Err(_) => Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    }
}

fn cache_alert(alert: &Alert) {
    let alert_clone = alert.clone();
    with_alert_store(|store| {
        store.insert(alert_clone.id.clone(), alert_clone);
    });
}

fn remove_alert_by_id(id: &str) -> Option<Alert> {
    with_alert_store(|store| store.remove(id))
}

fn format_timestamp_ms(ts_ms: i64) -> String {
    if let Some(datetime) = chrono::DateTime::<Utc>::from_timestamp_millis(ts_ms) {
        datetime
            .with_timezone(&Local)
            .format("%d/%m/%Y %H:%M:%S")
            .to_string()
    } else {
        Local::now().format("%d/%m/%Y %H:%M:%S").to_string()
    }
}

fn map_alert_type(source: &str) -> AlertType {
    match source {
        "Temperature out of range" => AlertType::TempUp,
        "Inactivity TimeOut" => AlertType::Disconnect,
        _ => AlertType::TempUp,
    }
}

fn map_description(source: &str, details: Option<&AlarmDetails>) -> String {
    match source {
        "Temperature out of range" => details
            .and_then(|d| d.data.clone())
            .unwrap_or_else(|| "Temperatura fuera de rango".to_string()),
        "Inactivity TimeOut" => "Dispositivo desconectado".to_string(),
        _ => "Detalle no disponible".to_string(),
    }
}

fn alert_from_params(params: &AlarmParams) -> Alert {
    Alert {
        id: params.id.value.clone(),
        date_time: format_timestamp_ms(params.created_time),
        alert_type: map_alert_type(&params.alarm_type),
        device: params.originator_name.clone(),
        description: map_description(&params.alarm_type, params.details.as_ref()),
    }
}

fn emit_alert_added(app_handle: &tauri::AppHandle, alert: &Alert) {
    if let Err(err) = app_handle.emit(ALERT_ADDED_EVENT, alert) {
        warn!(
            "[ALERT] No se pudo emitir evento de alerta agregada {}: {:?}",
            alert.id, err
        );
    }
}

fn emit_alert_removed(app_handle: &tauri::AppHandle, id: &str) {
    let payload = AlertRemovalEvent { id: id.to_string() };
    if let Err(err) = app_handle.emit(ALERT_REMOVED_EVENT, &payload) {
        warn!(
            "[ALERT] No se pudo emitir evento de alerta eliminada {}: {:?}",
            id, err
        );
    }
}

fn handle_active_alarm(params: AlarmParams, app_handle: &tauri::AppHandle) {
    let alert = alert_from_params(&params);
    info!(
        "[ALERT] ACTIVADA {} tipo={} dispositivo={}",
        alert.id, params.alarm_type, params.originator_name
    );
    cache_alert(&alert);
    handle_alert_activation_side_effects(app_handle);
    emit_alert_added(app_handle, &alert);
}

fn handle_cleared_alarm(params: AlarmParams, app_handle: &tauri::AppHandle) {
    let alert_id = params.id.value;
    if remove_alert_by_id(&alert_id).is_some() {
        info!(
            "[ALERT] LIBERADA {} tipo={} dispositivo={}",
            alert_id, params.alarm_type, params.originator_name
        );
        emit_alert_removed(app_handle, &alert_id);
        if !has_active_alerts() {
            handle_no_active_alerts(app_handle);
        }
    } else {
        debug!(
            "[ALERT] Se recibió CLEAR para {}, pero no existe en cache",
            alert_id
        );
    }
}

fn handle_rpc_payload(payload: &[u8], app_handle: &tauri::AppHandle) {
    let envelope: AlarmRpcEnvelope = match serde_json::from_slice(payload) {
        Ok(data) => data,
        Err(err) => {
            warn!("[MQTT] No se pudo parsear payload RPC: {:?}", err);
            return;
        }
    };

    if !envelope.method.eq_ignore_ascii_case("ALARM") {
        debug!(
            "[MQTT] Método RPC ignorado: {}",
            envelope.method
        );
        return;
    }

    match envelope.params.status {
        AlarmStatus::ActiveUnack => handle_active_alarm(envelope.params, app_handle),
        AlarmStatus::ClearedUnack => handle_cleared_alarm(envelope.params, app_handle),
        AlarmStatus::Unknown => {
            warn!("[MQTT] Estado de alarma no manejado, se ignora payload.");
        }
    }
}

fn handle_supabase_update(payload: &SupabaseUpdatePayload, app_handle: &tauri::AppHandle) {
    match validate_binary_array(&payload.new.message) {
        Ok(binary_array) => {
            let timestamp = parse_supabase_timestamp(&payload.commit_timestamp);
            let update = DeviceStatusUpdate {
                timestamp: timestamp.clone(),
                status: binary_array.clone(),
            };

            info!(
                "[SUPABASE] Estado actualizado: {:?} en {}",
                binary_array, timestamp
            );

            process_refrigerator_alarms(&binary_array, app_handle);

            if let Err(err) = app_handle.emit(DEVICE_STATUS_EVENT, &update) {
                warn!(
                    "[SUPABASE] No se pudo emitir evento de actualización: {:?}",
                    err
                );
            }
        }
        Err(err) => {
            error!("[SUPABASE] Validación fallida: {}. Mensaje: {}", err, payload.new.message);
        }
    }
}

fn process_refrigerator_alarms(binary_array: &[u8], app_handle: &tauri::AppHandle) {
    let store = REFRIGERATOR_ALARM_STATE.get_or_init(|| Mutex::new(vec![0; BINARY_ARRAY_SIZE]));
    let mut state_guard = store
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    
    let previous_state = state_guard.clone();
    *state_guard = binary_array.to_vec();
    
    for (index, (&current_value, &previous_value)) in binary_array.iter().zip(previous_state.iter()).enumerate() {
        if index >= REFRIGERATOR_NAMES.len() {
            break;
        }
        
        if current_value == previous_value {
            continue;
        }
        
        let device_name = REFRIGERATOR_NAMES[index];
        let alert_id = format!("refrigerator-temp-{}", index);
        
        if current_value == 1 && previous_value == 0 {
            let alert = Alert {
                id: alert_id.clone(),
                date_time: Local::now().format("%d/%m/%Y %H:%M:%S").to_string(),
                alert_type: AlertType::TempUp,
                device: device_name.to_string(),
                description: TEMPERATURE_ALARM_DESCRIPTION.to_string(),
            };
            
            info!(
                "[REFRIGERATOR] ACTIVADA {} tipo={} dispositivo={}",
                alert.id, TEMPERATURE_ALARM_TYPE, device_name
            );
            cache_alert(&alert);
            handle_alert_activation_side_effects(app_handle);
            emit_alert_added(app_handle, &alert);
        } else if current_value == 0 && previous_value == 1 {
            if remove_alert_by_id(&alert_id).is_some() {
                info!(
                    "[REFRIGERATOR] LIBERADA {} tipo={} dispositivo={}",
                    alert_id, TEMPERATURE_ALARM_TYPE, device_name
                );
                emit_alert_removed(app_handle, &alert_id);
                if !has_active_alerts() {
                    handle_no_active_alerts(app_handle);
                }
            }
        }
    }
}

#[tauri::command]
fn get_active_alerts() -> Vec<Alert> {
    snapshot_alerts()
}

#[tauri::command]
fn remove_alert(app_handle: tauri::AppHandle, id: String) -> bool {
    if remove_alert_by_id(&id).is_some() {
        emit_alert_removed(&app_handle, &id);
        if !has_active_alerts() {
            handle_no_active_alerts(&app_handle);
        }
        true
    } else {
        false
    }
}

#[tauri::command]
fn check_internet_connection() -> bool {
    TcpStream::connect_timeout(
        &"8.8.8.8:53".parse().unwrap(),
        std::time::Duration::from_secs(2),
    )
    .is_ok()
}

#[tauri::command]
fn get_mute_status() -> MuteStatePayload {
    snapshot_mute_state()
}

#[tauri::command]
fn toggle_alerts_mute(app_handle: tauri::AppHandle) -> MuteStatePayload {
    let currently_muted = with_mute_controller(|ctrl| ctrl.muted);

    if currently_muted {
        force_unmute(&app_handle);
        if has_active_alerts() {
            set_buzzer_state(true);
        } else {
            set_buzzer_state(false);
        }
        snapshot_mute_state()
    } else {
        if !has_active_alerts() {
            return snapshot_mute_state();
        }
        mute_alerts_internal(&app_handle)
    }
}

fn invalidate_buzzer_line() {
    if let Some(cache) = BUZZER_GPIO_CACHE.get() {
        let mut guard = cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = None;
    }
}

fn resolve_buzzer_line() -> Option<(String, String)> {
    if let Some(cache) = BUZZER_GPIO_CACHE.get() {
        if let Some(pair) = cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
        {
            return Some(pair);
        }
    }

    let gpiofind_output = match Command::new("gpiofind").arg("BUZZER_EN").output() {
        Ok(output) => output,
        Err(err) => {
            error!("[BUZZER] No se pudo ejecutar gpiofind: {:?}", err);
            return None;
        }
    };

    if !gpiofind_output.status.success() {
        error!(
            "[BUZZER] gpiofind devolvio codigo {:?}: {}",
            gpiofind_output.status.code(),
            String::from_utf8_lossy(&gpiofind_output.stderr)
        );
        return None;
    }

    let location = String::from_utf8_lossy(&gpiofind_output.stdout).to_string();
    let mut parts = location.split_whitespace();
    let chip = match parts.next() {
        Some(chip) => chip.trim().to_string(),
        None => {
            error!("[BUZZER] gpiofind no entrego chip valido");
            return None;
        }
    };
    let line = match parts.next() {
        Some(line) => line.trim().to_string(),
        None => {
            error!("[BUZZER] gpiofind no entrego linea valida");
            return None;
        }
    };

    let pair = (chip, line);
    let cache = buzzer_gpio_cache();
    let mut guard = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = Some(pair.clone());
    Some(pair)
}

/// Controla el estado del buzzer. Cuando se enciende, parpadea cada segundo.
fn set_buzzer_state(on: bool) -> bool {
    if !is_buzzer_enabled() {
        debug!("[BUZZER] Cambio de estado ignorado (deshabilitado)");
        if !on {
            let _ = stop_buzzer_blinking();
        }
        return true;
    }

    let result = if on {
        info!("[BUZZER] Activado");
        start_buzzer_blinking()
    } else {
        info!("[BUZZER] Desactivado");
        stop_buzzer_blinking()
    };

    if !result {
        error!(
            "[BUZZER] No se pudo cambiar estado a {}",
            if on { "ON" } else { "OFF" }
        );
    }

    result
}

fn start_buzzer_blinking() -> bool {
    if with_buzzer_controller(|ctrl| ctrl.handle.is_some()) {
        return true;
    }

    if !set_buzzer_gpio(true) {
        return false;
    }

    let handle = async_runtime::spawn(async move {
        let mut level = false;
        let mut consecutive_failures: u8 = 0;
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if is_shutting_down() {
                break;
            }
            level = !level;
            if set_buzzer_gpio(level) {
                consecutive_failures = 0;
            } else {
                consecutive_failures = consecutive_failures.saturating_add(1);
                warn!("[BUZZER] Fallo al alternar nivel {}", level as u8);
                if consecutive_failures >= BUZZER_FAILURE_LIMIT {
                    error!(
                        "[BUZZER] Se desactiva parpadeo tras {} errores consecutivos",
                        BUZZER_FAILURE_LIMIT
                    );
                    break;
                }
            }
        }

        let _ = set_buzzer_gpio(false);
    });

    let mut handle_slot = Some(handle);
    let was_set = with_buzzer_controller(|ctrl| {
        if ctrl.handle.is_none() {
            ctrl.handle = handle_slot.take();
            true
        } else {
            false
        }
    });

    if !was_set {
        if let Some(handle) = handle_slot {
            handle.abort();
        }
    }

    true
}

fn stop_buzzer_blinking() -> bool {
    if let Some(handle) = with_buzzer_controller(|ctrl| ctrl.handle.take()) {
        handle.abort();
    }

    set_buzzer_gpio(false)
}

fn set_buzzer_gpio(on: bool) -> bool {
    let level = if on { "1" } else { "0" };

    let (chip, line) = match resolve_buzzer_line() {
        Some(pair) => pair,
        None => return false,
    };

    match Command::new("gpioset")
        .arg(&chip)
        .arg(format!("{}={}", line, level))
        .status()
    {
        Ok(status) if status.success() => true,
        Ok(status) => {
            error!("[BUZZER] gpioset termino con codigo {:?}", status.code());
            invalidate_buzzer_line();
            false
        }
        Err(err) => {
            error!("[BUZZER] No se pudo ejecutar gpioset: {:?}", err);
            invalidate_buzzer_line();
            false
        }
    }
}

fn request_shutdown() {
    if !SHUTDOWN.swap(true, Ordering::SeqCst) {
        info!("[CORE] Shutdown solicitado");
    }
    MQTT_CONNECTED.store(false, Ordering::SeqCst);
    SUPABASE_CONNECTED.store(false, Ordering::SeqCst);
    let _ = stop_buzzer_blinking();
}


fn build_mqtt_options() -> Option<MqttOptions> {
    let cfg = app_config();
    let mut mqttoptions = MqttOptions::new(
        cfg.mqtt_client_id.as_str(),
        cfg.mqtt_server.as_str(),
        cfg.mqtt_port,
    );
    mqttoptions.set_credentials(cfg.mqtt_username.as_str(), cfg.mqtt_password.as_str());
    mqttoptions.set_keep_alive(Duration::from_secs(60));

    if cfg.mqtt_use_secure_client {
        let ca_path = "certs/emqxsl-ca.crt";
        let ca_bytes = match fs::read(ca_path) {
            Ok(b) => b,
            Err(e) => {
                error!("[MQTT] No se pudo leer CA en {}: {:?}", ca_path, e);
                return None;
            }
        };
        let tls_cfg = TlsConfiguration::Simple {
            ca: ca_bytes,
            alpn: Some(vec![b"mqtt".to_vec()]),
            client_auth: None,
        };
        mqttoptions.set_transport(Transport::tls_with_config(tls_cfg));
    }

    Some(mqttoptions)
}

fn start_mqtt_loop(app_handle: tauri::AppHandle) {
    if let Err(err) = thread::Builder::new()
        .name("mqtt-loop".to_string())
        .spawn(move || {
            let mut retry_delay = MQTT_RETRY_DELAY;
            while !is_shutting_down() {
                MQTT_CONNECTED.store(false, Ordering::SeqCst);

                let Some(mqttoptions) = build_mqtt_options() else {
                    error!(
                        "[MQTT] No se pudieron construir las opciones MQTT. Reintentando en {:?}...",
                        retry_delay
                    );
                    sleep_with_shutdown(retry_delay);
                    retry_delay = next_retry_delay(retry_delay);
                    continue;
                };

                let cfg = app_config();
                info!(
                    "[MQTT] Intentando conectar ({}) con {}:{} como {}",
                    if cfg.mqtt_use_secure_client { "TLS" } else { "TCP" },
                    cfg.mqtt_server.as_str(),
                    cfg.mqtt_port,
                    cfg.mqtt_client_id.as_str()
                );

                let (client, mut connection) = Client::new(mqttoptions, 10);

                if let Err(err) = client.subscribe(MQTT_RPC_REQUEST_TOPIC, QoS::AtLeastOnce) {
                    error!(
                        "[MQTT] No se pudo suscribir a {}: {:?}. Reintentando en {:?}...",
                        MQTT_RPC_REQUEST_TOPIC, err, retry_delay
                    );
                    sleep_with_shutdown(retry_delay);
                    retry_delay = next_retry_delay(retry_delay);
                    continue;
                }

                info!(
                    "[MQTT] Suscrito a solicitudes RPC en {}",
                    MQTT_RPC_REQUEST_TOPIC
                );
                retry_delay = MQTT_RETRY_DELAY;

                for event in connection.iter() {
                    if is_shutting_down() {
                        info!("[MQTT] Loop detenido por shutdown");
                        break;
                    }

                    match event {
                        Ok(Event::Incoming(Packet::Publish(publish))) => {
                            MQTT_CONNECTED.store(true, Ordering::SeqCst);
                            handle_rpc_payload(&publish.payload, &app_handle);
                        }
                        Ok(Event::Incoming(pkt)) => {
                            MQTT_CONNECTED.store(true, Ordering::SeqCst);
                            debug!("[MQTT] Evento entrante: {:?}", pkt);
                        }
                        Ok(Event::Outgoing(pkt)) => {
                            debug!("[MQTT] Evento saliente: {:?}", pkt);
                        }
                        Err(e) => {
                            error!("[MQTT] Error en loop: {:?}", e);
                            MQTT_CONNECTED.store(false, Ordering::SeqCst);
                            break;
                        }
                    }
                }

                if is_shutting_down() {
                    break;
                }

                warn!(
                    "[MQTT] Loop MQTT finalizado. Reintentando en {:?}...",
                    retry_delay
                );

                sleep_with_shutdown(retry_delay);
                retry_delay = next_retry_delay(retry_delay);
            }

            info!("[MQTT] Loop terminado");
        })
    {
        error!("[MQTT] No se pudo iniciar hilo de conexion: {:?}", err);
    }
}

#[tauri::command]
fn is_mqtt_connected() -> bool {
    MQTT_CONNECTED.load(Ordering::SeqCst)
}

#[tauri::command]
fn is_supabase_connected() -> bool {
    SUPABASE_CONNECTED.load(Ordering::SeqCst)
}

fn start_supabase_loop(app_handle: tauri::AppHandle) {
    let cfg = app_config();
    
    if cfg.supabase_url.is_empty() || cfg.supabase_anon_key.is_empty() {
        info!("[SUPABASE] No configurado, omitiendo inicialización");
        return;
    }

    let supabase_url = cfg.supabase_url.clone();
    let supabase_key = cfg.supabase_anon_key.clone();

    if let Err(err) = thread::Builder::new()
        .name("supabase-loop".to_string())
        .spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap_or_else(|e| {
                error!("[SUPABASE] No se pudo crear runtime: {:?}", e);
                panic!("Runtime error");
            });

            rt.block_on(async {
                let mut retry_delay = SUPABASE_RETRY_DELAY;

                while !is_shutting_down() {
                    SUPABASE_CONNECTED.store(false, Ordering::SeqCst);

                    let realtime_url = supabase_url
                        .replace("https://", "wss://")
                        .replace("http://", "ws://");
                    let realtime_url = format!("{}/realtime/v1", realtime_url);

                    info!(
                        "[SUPABASE] Conectando a {}",
                        realtime_url
                    );

                    let client = match RealtimeClient::new(
                        &realtime_url,
                        RealtimeClientOptions {
                            api_key: supabase_key.clone(),
                            ..Default::default()
                        },
                    ) {
                        Ok(c) => c,
                        Err(err) => {
                            error!(
                                "[SUPABASE] No se pudo crear cliente: {:?}. Reintentando en {:?}...",
                                err, retry_delay
                            );
                            tokio::time::sleep(retry_delay).await;
                            retry_delay = (retry_delay * 2).min(SUPABASE_MAX_RETRY_DELAY);
                            continue;
                        }
                    };

                    if let Err(err) = client.connect().await {
                        error!(
                            "[SUPABASE] No se pudo conectar: {:?}. Reintentando en {:?}...",
                            err, retry_delay
                        );
                        tokio::time::sleep(retry_delay).await;
                        retry_delay = (retry_delay * 2).min(SUPABASE_MAX_RETRY_DELAY);
                        continue;
                    }

                    info!("[SUPABASE] Conectado exitosamente");
                    SUPABASE_CONNECTED.store(true, Ordering::SeqCst);
                    retry_delay = SUPABASE_RETRY_DELAY;

                    let channel = client.channel(SUPABASE_CHANNEL_NAME, Default::default()).await;
                    let filter = PostgresChangesFilter::new(PostgresChangeEvent::Update, SUPABASE_DB_SCHEMA);
                    let mut rx = channel.on_postgres_changes(filter).await;

                    if let Err(err) = channel.subscribe().await {
                        error!("[SUPABASE] Error al suscribirse: {:?}", err);
                        SUPABASE_CONNECTED.store(false, Ordering::SeqCst);
                        tokio::time::sleep(retry_delay).await;
                        retry_delay = (retry_delay * 2).min(SUPABASE_MAX_RETRY_DELAY);
                        continue;
                    }

                    info!(
                        "[SUPABASE] Suscrito a cambios UPDATE en {}",
                        SUPABASE_DB_SCHEMA
                    );

                    let mut should_reconnect = false;
                    while !is_shutting_down() {
                        match tokio::time::timeout(Duration::from_secs(2), rx.recv()).await {
                            Ok(Some(change)) => {
                                if let Ok(json_str) = serde_json::to_string(&change) {
                                    if let Ok(payload) = serde_json::from_str::<SupabaseUpdatePayload>(&json_str) {
                                        handle_supabase_update(&payload, &app_handle);
                                    } else {
                                        debug!("[SUPABASE] Payload deserializado incorrectamente");
                                    }
                                }
                            }
                            Ok(None) => {
                                warn!("[SUPABASE] Conexión cerrada por servidor");
                                should_reconnect = true;
                                break;
                            }
                            Err(_) => {
                                if is_shutting_down() {
                                    should_reconnect = false;
                                    break;
                                }
                            }
                        }
                    }

                    if is_shutting_down() {
                        SUPABASE_CONNECTED.store(false, Ordering::SeqCst);
                        info!("[SUPABASE] Shutdown finalizado");
                        break;
                    }

                    if should_reconnect {
                        warn!(
                            "[SUPABASE] Reconectando en {:?}...",
                            retry_delay
                        );
                        tokio::time::sleep(retry_delay).await;
                        retry_delay = (retry_delay * 2).min(SUPABASE_MAX_RETRY_DELAY);
                    }
                }

                info!("[SUPABASE] Loop terminado");
                SUPABASE_CONNECTED.store(false, Ordering::SeqCst);
            });

            rt.shutdown_timeout(Duration::from_secs(1));
        })
    {
        error!("[SUPABASE] No se pudo iniciar hilo de conexión: {:?}", err);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .on_window_event(|_, event| match event {
            WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed => {
                request_shutdown();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_active_alerts,
            remove_alert,
            check_internet_connection,
            get_mute_status,
            toggle_alerts_mute,
            is_mqtt_connected,
            is_supabase_connected
        ])
        .setup(|app| {
            let app_handle = app.handle();
            start_mqtt_loop(app_handle.clone());
            start_supabase_loop(app_handle.clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
