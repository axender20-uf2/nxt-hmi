use chrono::{Local, SecondsFormat, Utc};
use rumqttc::{Client, Event, MqttOptions, Packet, QoS, TlsConfiguration, Transport};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::TcpStream;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime};
use tauri::async_runtime::{self, JoinHandle};
use tauri::Emitter;

static ALERT_STORE: OnceLock<Mutex<HashMap<String, Alert>>> = OnceLock::new();
const ALERT_ADDED_EVENT: &str = "alerts://added";
const ALERT_REMOVED_EVENT: &str = "alerts://removed";
static MUTE_CONTROLLER: OnceLock<Mutex<MuteController>> = OnceLock::new();
const MUTE_CHANGED_EVENT: &str = "alerts://mute_changed";
const MUTE_DURATION: Duration = Duration::from_secs(600);

static MQTT_CONNECTED: AtomicBool = AtomicBool::new(false);
const MQTT_RETRY_DELAY: Duration = Duration::from_secs(5);

// Configuración de conexión MQTT (alineada con MQTTX)
pub const MQTT_SERVER: &str = "j0661b06.ala.us-east-1.emqxsl.com";
pub const MQTT_PORT: u16 = 8883;
pub const MQTT_CLIENT_ID: &str = "hmi-cli";
pub const MQTT_USERNAME: &str = "test";
pub const MQTT_PASSWORD: &str = "test";
pub const MQTT_RPC_REQUEST_TOPIC: &str = "v1/devices/me/rpc/request/+";

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

#[derive(Debug, Serialize)]
struct AlertRemovalEvent {
    id: String,
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
        eprintln!("[MUTE] No se pudo emitir estado mute: {:?}", err);
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
        tokio::time::sleep(MUTE_DURATION).await;
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
        .checked_add(MUTE_DURATION)
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
        eprintln!(
            "[MQTT] No se pudo emitir evento de alerta agregada: {:?}",
            err
        );
    }
}

fn emit_alert_removed(app_handle: &tauri::AppHandle, id: &str) {
    let payload = AlertRemovalEvent { id: id.to_string() };
    if let Err(err) = app_handle.emit(ALERT_REMOVED_EVENT, &payload) {
        eprintln!(
            "[MQTT] No se pudo emitir evento de alerta eliminada: {:?}",
            err
        );
    }
}

fn handle_active_alarm(params: AlarmParams, app_handle: &tauri::AppHandle) {
    let alert = alert_from_params(&params);
    cache_alert(&alert);
    handle_alert_activation_side_effects(app_handle);
    emit_alert_added(app_handle, &alert);
}

fn handle_cleared_alarm(params: AlarmParams, app_handle: &tauri::AppHandle) {
    let alert_id = params.id.value;
    if remove_alert_by_id(&alert_id).is_some() {
        emit_alert_removed(app_handle, &alert_id);
        if !has_active_alerts() {
            handle_no_active_alerts(app_handle);
        }
    }
}

fn handle_rpc_payload(payload: &[u8], app_handle: &tauri::AppHandle) {
    let envelope: AlarmRpcEnvelope = match serde_json::from_slice(payload) {
        Ok(data) => data,
        Err(err) => {
            eprintln!("[MQTT] No se pudo parsear payload RPC: {:?}", err);
            return;
        }
    };

    if !envelope.method.eq_ignore_ascii_case("ALARM") {
        return;
    }

    match envelope.params.status {
        AlarmStatus::ActiveUnack => handle_active_alarm(envelope.params, app_handle),
        AlarmStatus::ClearedUnack => handle_cleared_alarm(envelope.params, app_handle),
        AlarmStatus::Unknown => {
            println!("[MQTT] Estado de alarma no manejado, se ignora payload.");
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

/// Ejecuta la línea de comandos documentada para fijar el estado del buzzer.
fn set_buzzer_state(on: bool) -> bool {
    let level = if on { "1" } else { "0" };

    let gpiofind_output = match Command::new("gpiofind").arg("BUZZER_EN").output() {
        Ok(output) => output,
        Err(err) => {
            eprintln!("[BUZZER] No se pudo ejecutar gpiofind: {:?}", err);
            return false;
        }
    };

    if !gpiofind_output.status.success() {
        eprintln!(
            "[BUZZER] gpiofind devolvió código {:?}: {}",
            gpiofind_output.status.code(),
            String::from_utf8_lossy(&gpiofind_output.stderr)
        );
        return false;
    }

    let location = String::from_utf8_lossy(&gpiofind_output.stdout);
    let mut parts = location.split_whitespace();
    let chip = match parts.next() {
        Some(chip) => chip.trim(),
        None => {
            eprintln!("[BUZZER] gpiofind no devolvió un chip válido");
            return false;
        }
    };
    let line = match parts.next() {
        Some(line) => line.trim(),
        None => {
            eprintln!("[BUZZER] gpiofind no devolvió una línea válida");
            return false;
        }
    };

    match Command::new("gpioset")
        .arg(chip)
        .arg(format!("{}={}", line, level))
        .status()
    {
        Ok(status) if status.success() => true,
        Ok(status) => {
            eprintln!("[BUZZER] gpioset terminó con código {:?}", status.code());
            false
        }
        Err(err) => {
            eprintln!("[BUZZER] No se pudo ejecutar gpioset: {:?}", err);
            false
        }
    }
}

fn build_mqtt_options() -> Option<MqttOptions> {
    let ca_path = "certs/emqxsl-ca.crt";
    let ca_bytes = match fs::read(ca_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[MQTT] No se pudo leer CA en {}: {:?}", ca_path, e);
            return None;
        }
    };

    let mut mqttoptions = MqttOptions::new(MQTT_CLIENT_ID, MQTT_SERVER, MQTT_PORT);
    mqttoptions.set_credentials(MQTT_USERNAME, MQTT_PASSWORD);
    mqttoptions.set_keep_alive(Duration::from_secs(60));
    let tls_cfg = TlsConfiguration::Simple {
        ca: ca_bytes,
        alpn: Some(vec![b"mqtt".to_vec()]),
        client_auth: None,
    };
    mqttoptions.set_transport(Transport::tls_with_config(tls_cfg));

    Some(mqttoptions)
}

fn start_mqtt_loop(app_handle: tauri::AppHandle) {
    thread::spawn(move || loop {
        MQTT_CONNECTED.store(false, Ordering::SeqCst);

        let Some(mqttoptions) = build_mqtt_options() else {
            eprintln!(
                "[MQTT] No se pudieron construir las opciones MQTT. Reintentando en {:?}...",
                MQTT_RETRY_DELAY
            );
            thread::sleep(MQTT_RETRY_DELAY);
            continue;
        };

        println!(
            "[MQTT] Intentando conectar (TLS) con {}:{} como {}",
            MQTT_SERVER, MQTT_PORT, MQTT_CLIENT_ID
        );

        let (client, mut connection) = Client::new(mqttoptions, 10);

        if let Err(err) = client.subscribe(MQTT_RPC_REQUEST_TOPIC, QoS::AtLeastOnce) {
            eprintln!(
                "[MQTT] No se pudo suscribir a {}: {:?}. Reintentando en {:?}...",
                MQTT_RPC_REQUEST_TOPIC, err, MQTT_RETRY_DELAY
            );
            thread::sleep(MQTT_RETRY_DELAY);
            continue;
        }

        println!(
            "[MQTT] Suscrito a solicitudes RPC en {}",
            MQTT_RPC_REQUEST_TOPIC
        );

        for event in connection.iter() {
            match event {
                Ok(Event::Incoming(Packet::Publish(publish))) => {
                    MQTT_CONNECTED.store(true, Ordering::SeqCst);
                    handle_rpc_payload(&publish.payload, &app_handle);
                }
                Ok(Event::Incoming(pkt)) => {
                    MQTT_CONNECTED.store(true, Ordering::SeqCst);
                    println!("[MQTT] Evento entrante: {:?}", pkt);
                }
                Ok(Event::Outgoing(pkt)) => {
                    println!("[MQTT] Evento saliente: {:?}", pkt);
                }
                Err(e) => {
                    eprintln!("[MQTT] Error en loop: {:?}", e);
                    MQTT_CONNECTED.store(false, Ordering::SeqCst);
                    break;
                }
            }
        }

        eprintln!(
            "[MQTT] Loop MQTT finalizado. Reintentando en {:?}...",
            MQTT_RETRY_DELAY
        );

        thread::sleep(MQTT_RETRY_DELAY);
    });
}

#[tauri::command]
fn is_mqtt_connected() -> bool {
    MQTT_CONNECTED.load(Ordering::SeqCst)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_active_alerts,
            remove_alert,
            check_internet_connection,
            get_mute_status,
            toggle_alerts_mute,
            is_mqtt_connected
        ])
        .setup(|app| {
            start_mqtt_loop(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
