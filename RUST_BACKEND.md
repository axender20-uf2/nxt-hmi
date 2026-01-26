# Backend Rust - Sistema de Gestión de Alertas

## Propósito General

Este archivo (`src-tauri/src/lib.rs`) es el **backend Rust de una aplicación de monitoreo de alertas** (HMI) que se ejecuta dentro de Tauri. Actúa como intermediario entre un servidor MQTT (que envía alertas de dispositivos) y la interfaz gráfica del usuario, gestionando alertas de temperatura y desconexiones de dispositivos, además de controlar hardware como un buzzer.

---

## Funciones Principales

### 1. **Conexión MQTT**
- Se conecta a un servidor MQTT configurado
- Escucha continuamente mensajes de alarma en tiempo real
- Si falla la conexión, reintenta automáticamente con retardos progresivos
- Implementa mecanismo de backoff exponencial (máximo 60 segundos)
- Soporta conexiones tanto TCP como TLS/SSL

### 2. **Gestión de Alertas**
- Recibe alarmas del servidor (desconexiones, temperaturas fuera de rango)
- Las almacena en memoria mediante un HashMap centralizado
- Las comunica a la interfaz gráfica mediante eventos Tauri
- Permite consultar el listado de alertas activas
- Permite eliminar alertas manualmente
- Emite eventos cuando se agregan o se eliminan alertas

### 3. **Control del Buzzer (Alarma Sonora)**
- Enciende/apaga un buzzer físico a través de GPIO del hardware
- Hace parpadear el buzzer cada segundo cuando está activo
- Detiene automáticamente si se resuelven todas las alertas
- Implementa caché para optimizar búsqueda de líneas GPIO
- Maneja fallos consecutivos y desactiva si se alcanzan límites

### 4. **Sistema de Silenciamiento (Mute)**
- Permite silenciar alertas por un tiempo configurable (default: 10 minutos)
- Auto-desmute cuando ese tiempo expira
- Se reactiva el buzzer si llegan nuevas alertas mientras está silenciado
- Emite eventos de cambio de estado a la interfaz

### 5. **Configuración y Logging**
- Lee configuración desde un archivo YAML (`src-tauri/config/config.yaml`)
- Crea configuración por defecto si no existe
- Registra todos los eventos en logs detallados para debugging
- Los logs incluyen timestamp, nivel y mensaje

---

## Flujo de Datos

### Fuente 1: MQTT (Alarmas en Tiempo Real)
```
Servidor MQTT
     ↓
Recepción de alarmas (formato RPC JSON)
     ↓
Parseo y validación del payload
     ↓
Almacenamiento en HashMap (ALERT_STORE)
     ↓
Efectos secundarios:
  - Control del buzzer
  - Desactivación del mute (si está activo)
     ↓
Emisión de eventos a la interfaz gráfica
```

### Fuente 2: Supabase Realtime (Actualizaciones de Base de Datos)
```
Supabase Realtime WebSocket
     ↓
Escucha eventos UPDATE en schema public
     ↓
Recepción de cambios en JSON con array [x,x,x,x,x,x]
     ↓
Parseo y validación del array (6 binarios: 0 o 1)
     ↓
Comparación con estado anterior para detectar cambios
     ↓
Por cada cambio:
  - 0→1: Crear alarma "Temperature out of range" (ACTIVE_UNACK)
  - 1→0: Eliminar alarma (CLEARED_UNACK)
     ↓
Procesamiento mediante las mismas funciones que MQTT
     ↓
Emisión de eventos a la interfaz gráfica
```

---

## Comandos Disponibles (Tauri Commands)

Estos comandos pueden ser invocados desde la interfaz frontend:

| Comando | Descripción |
|---------|-------------|
| `get_active_alerts()` | Retorna la lista de alertas activas |
| `remove_alert(id)` | Elimina una alerta por ID |
| `check_internet_connection()` | Verifica conexión a internet (ping a 8.8.8.8) |
| `get_mute_status()` | Obtiene el estado actual del mute |
| `toggle_alerts_mute()` | Alterna el estado de silenciamiento |
| `is_mqtt_connected()` | Verifica si está conectado al broker MQTT |
| `is_supabase_connected()` | Verifica si está conectado a Supabase Realtime |

---

## Eventos Emitidos

La aplicación emite los siguientes eventos a la interfaz:

| Evento | Payload | Descripción |
|--------|---------|-------------|
| `alerts://added` | Alert | Se activó una nueva alerta |
| `alerts://removed` | AlertRemovalEvent | Se eliminó una alerta |
| `alerts://mute_changed` | MuteStatePayload | Cambió el estado del mute |
| `device://status_changed` | DeviceStatusUpdate | Se actualizó el estado de un dispositivo desde Supabase |

---

## Configuración

El archivo `src-tauri/src-tauri/config/config.yaml` contiene:

```yaml
MQTT_SERVER: j0661b06.ala.us-east-1.emqxsl.com
MQTT_USE_SECURE_CLIENT: true
MQTT_PORT: 8883
MQTT_CLIENT_ID: hmi-cli
MQTT_USERNAME: test
MQTT_PASSWORD: test
MUTE_DURATION: 600  # segundos
BUZZER_ENABLED: true
SUPABASE_URL: https://tu-proyecto.supabase.co  # Opcional
SUPABASE_ANON_KEY: tu-api-key-aqui  # Opcional
```

**Nota**: Las credenciales de Supabase son opcionales. Si no se proporcionan, el loop de Supabase no se iniciará.

---

## Tipos de Alertas Soportadas

### MQTT (ThingsBoard)
- **Temperature out of range** → Temperatura fuera de rango (TempUp/TempDown)
- **Inactivity TimeOut** → Dispositivo desconectado (Disconnect)

### Supabase (Refrigeradores)
Array [x,x,x,x,x,x] donde cada posición representa:
1. **Bodega - microbiología refri 2**
2. **Bodega - microbiología refri 1**
3. **Bodega - química refri 1**
4. **Bodega - banco de sangre**
5. **Bodega - química refri 2**
6. **Bodega - Inmunología refri 1**

Cada valor:
- **0** = Alarma eliminada (CLEARED_UNACK)
- **1** = Alarma activa (ACTIVE_UNACK)
- Tipo: siempre "Temperature out of range"
- Descripción: "Temperatura fuera de rango 2 - 8 °C"

---

## Estructuras de Datos Clave

### Alert
Representa una alerta en el sistema
- `id`: Identificador único
- `date_time`: Timestamp de creación
- `alert_type`: Tipo de alerta (Disconnect, TempUp, TempDown)
- `device`: Nombre del dispositivo origen
- `description`: Descripción del evento

### MuteStatePayload
Estado del sistema de silenciamiento
- `muted`: ¿Está actualmente silenciado?
- `expires_at`: Cuándo expira el mute (formato RFC3339)

### DeviceStatusUpdate
Actualización de estado del dispositivo desde Supabase
- `timestamp`: Timestamp de la actualización (zona horaria Guatemala)
- `status`: Array de 6 valores binarios (0 o 1)

---

## Ciclo de Vida

1. **Inicialización**: Se ejecuta `init_logging()` y se construye el builder de Tauri
2. **Setup**: Se inician los loops de MQTT y Supabase en hilos separados
3. **Operación**: 
   - Loop MQTT escucha eventos de alarmas continuamente
   - Loop Supabase escucha cambios UPDATE en la base de datos
4. **Shutdown**: Cuando se cierra la ventana, se detienen ambos loops y el buzzer

---

## Reintentos y Resiliencia

### MQTT
- Reintento inicial: 5 segundos
- Máximo reintento: 60 segundos
- Backoff exponencial (duplica el tiempo entre reintentos)

### Supabase
- Reintento inicial: 5 segundos
- Máximo reintento: 60 segundos
- Backoff exponencial (duplica el tiempo entre reintentos)
- Timeout de 60 segundos por evento

---

## Características de Robustez

- ✅ Manejo de errores exhaustivo con logs detallados
- ✅ Reintentos automáticos de conexión MQTT y Supabase
- ✅ Graceful shutdown al cerrar la aplicación
- ✅ Caché de GPIO para optimizar acceso al hardware
- ✅ Sincronización thread-safe con Mutex y OnceLock
- ✅ Detección de corrupción de locks y recuperación automática
- ✅ Límite de fallos consecutivos del buzzer para prevenir bucles infinitos
- ✅ Validación exhaustiva de payloads (estructura, tipos, rangos)
- ✅ Conversión de zonas horarias (Supabase UTC → Guatemala GMT-6)
- ✅ Timeouts cortos (2s) para respuesta rápida a shutdown
- ✅ Dos fuentes de datos independientes sin interferencias
- ✅ Terminación limpia de runtime de Tokio (1s timeout)
