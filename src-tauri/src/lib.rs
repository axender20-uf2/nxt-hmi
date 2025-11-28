use rumqttc::{Client, Event, MqttOptions, TlsConfiguration, Transport};
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

static MQTT_CONNECTED: AtomicBool = AtomicBool::new(false);
const MQTT_RETRY_DELAY: Duration = Duration::from_secs(5);

// Configuración de conexión MQTT (alineada con MQTTX)
pub const MQTT_SERVER: &str = "rfc7cf00.ala.us-east-1.emqxsl.com";
pub const MQTT_PORT: u16 = 8883;
pub const MQTT_CLIENT_ID: &str = "hmi-cli";
pub const MQTT_USERNAME: &str = "test";
pub const MQTT_PASSWORD: &str = "test";

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

#[tauri::command]
fn get_mock_alerts() -> Vec<Alert> {
    vec![
        Alert {
            id: "1".to_string(),
            date_time: "10/11/2025 14:23:15".to_string(),
            alert_type: AlertType::Disconnect,
            device: "Zona A - Almacén".to_string(),
            description: "Sin conexión".to_string(),
        },
        Alert {
            id: "2".to_string(),
            date_time: "10/11/2025 15:45:32".to_string(),
            alert_type: AlertType::TempUp,
            device: "Zona B - Producción".to_string(),
            description: "Temp. alta 28°C".to_string(),
        },
        Alert {
            id: "3".to_string(),
            date_time: "10/11/2025 16:12:08".to_string(),
            alert_type: AlertType::TempDown,
            device: "Zona C - Laboratorio".to_string(),
            description: "Temp. baja -85°C".to_string(),
        },
        Alert {
            id: "4".to_string(),
            date_time: "09/11/2025 17:30:41".to_string(),
            alert_type: AlertType::Disconnect,
            device: "Zona D - Oficinas".to_string(),
            description: "Falla de red".to_string(),
        },
        Alert {
            id: "5".to_string(),
            date_time: "09/11/2025 18:05:19".to_string(),
            alert_type: AlertType::TempUp,
            device: "Zona E - Exterior".to_string(),
            description: "Sobrecalentami.".to_string(),
        },
    ]
}

#[tauri::command]
fn check_internet_connection() -> bool {
    TcpStream::connect_timeout(
        &"8.8.8.8:53".parse().unwrap(),
        std::time::Duration::from_secs(2),
    )
    .is_ok()
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

fn start_mqtt_loop() {
    thread::spawn(|| loop {
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

        let (_client, mut connection) = Client::new(mqttoptions, 10);

        for event in connection.iter() {
            match event {
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
    // Iniciar bucle MQTT en segundo plano
    start_mqtt_loop();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_mock_alerts,
            check_internet_connection,
            is_mqtt_connected
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
