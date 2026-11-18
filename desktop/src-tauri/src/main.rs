use std::{collections::BTreeMap, sync::Arc, time::Duration};

use futures::StreamExt;
use openradlr_bluetooth::{AdapterState, BluetoothAdapter, BluezAdapter, CharacteristicId};
use serde::Serialize;
use tauri::{async_runtime::Mutex, State, WebviewUrl, WebviewWindowBuilder};

const DEFAULT_SERVER_URL: &str = "https://app.openradlr.com";

fn server_url() -> &'static str {
    option_env!("OPENRADLR_DESKTOP_URL").unwrap_or(DEFAULT_SERVER_URL)
}

struct BluetoothState(Mutex<Option<Arc<BluezAdapter>>>);

async fn adapter(state: &BluetoothState) -> Result<Arc<BluezAdapter>, String> {
    if let Some(adapter) = state.0.lock().await.clone() {
        return Ok(adapter);
    }
    let adapter = Arc::new(
        BluezAdapter::open()
            .await
            .map_err(|error| error.to_string())?,
    );
    *state.0.lock().await = Some(adapter.clone());
    Ok(adapter)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Device {
    device_id: String,
    name: Option<String>,
}

#[derive(Serialize)]
struct Service {
    uuid: String,
    characteristics: Vec<Characteristic>,
}

#[derive(Serialize)]
struct Characteristic {
    uuid: String,
}

#[tauri::command]
async fn bluetooth_initialize(state: State<'_, BluetoothState>) -> Result<(), String> {
    adapter(&state).await.map(|_| ())
}

#[tauri::command]
async fn bluetooth_is_enabled(state: State<'_, BluetoothState>) -> Result<bool, String> {
    Ok(matches!(
        adapter(&state)
            .await?
            .state()
            .await
            .map_err(|error| error.to_string())?,
        AdapterState::Ready { .. }
    ))
}

#[tauri::command]
async fn bluetooth_devices(state: State<'_, BluetoothState>) -> Result<Vec<Device>, String> {
    adapter(&state)
        .await?
        .devices()
        .await
        .map_err(|error| error.to_string())
        .map(|devices| {
            devices
                .into_iter()
                .map(|device| Device {
                    device_id: device.id,
                    name: device.name,
                })
                .collect()
        })
}

#[tauri::command]
async fn bluetooth_scan(
    window: tauri::WebviewWindow,
    state: State<'_, BluetoothState>,
    services: Vec<String>,
    name: Option<String>,
    name_prefix: Option<String>,
    scan_id: String,
) -> Result<Vec<Device>, String> {
    let mut discoveries = adapter(&state)
        .await?
        .discover(&services)
        .await
        .map_err(|error| error.to_string())?;
    let mut devices = BTreeMap::new();
    while let Ok(Some(result)) =
        tokio::time::timeout(Duration::from_secs(3), discoveries.next()).await
    {
        let device = result.map_err(|error| error.to_string())?;
        if let Some(name) = name.as_deref() {
            if device.name.as_deref() != Some(name) {
                continue;
            }
        }
        if let Some(prefix) = name_prefix.as_deref() {
            if !device
                .name
                .as_deref()
                .is_some_and(|candidate| candidate.starts_with(prefix))
            {
                continue;
            }
        }
        // BlueZ puts cached devices in the stream even when they did not match its
        // discovery filter. Their advertised UUIDs are the final authority here.
        if !services.is_empty()
            && !device.services.iter().any(|service| {
                services
                    .iter()
                    .any(|wanted| service.eq_ignore_ascii_case(wanted))
            })
        {
            continue;
        }
        let candidate = Device {
            device_id: device.id.clone(),
            name: device.name,
        };
        let payload = serde_json::json!({ "scanId": scan_id, "device": candidate });
        let _ = window.eval(format!("window.OpenRadlrDesktop._scanDevice({payload});"));
        devices.insert(device.id.clone(), candidate);
    }
    let payload = serde_json::json!({ "scanId": scan_id });
    let _ = window.eval(format!("window.OpenRadlrDesktop._scanFinished({payload});"));
    Ok(devices.into_values().collect())
}

#[tauri::command]
async fn bluetooth_connect(
    state: State<'_, BluetoothState>,
    device_id: String,
) -> Result<(), String> {
    adapter(&state)
        .await?
        .connect(&device_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn bluetooth_disconnect(
    state: State<'_, BluetoothState>,
    device_id: String,
) -> Result<(), String> {
    adapter(&state)
        .await?
        .disconnect(&device_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn bluetooth_services(
    state: State<'_, BluetoothState>,
    device_id: String,
) -> Result<Vec<Service>, String> {
    adapter(&state)
        .await?
        .services(&device_id)
        .await
        .map_err(|error| error.to_string())
        .map(|services| {
            services
                .into_iter()
                .map(|service| Service {
                    uuid: service.uuid,
                    characteristics: service
                        .characteristics
                        .into_iter()
                        .map(|uuid| Characteristic { uuid })
                        .collect(),
                })
                .collect()
        })
}

async fn write(
    state: State<'_, BluetoothState>,
    device_id: String,
    service: String,
    characteristic: String,
    value: String,
    with_response: bool,
) -> Result<(), String> {
    let value = hex::decode(value).map_err(|error| error.to_string())?;
    let id = CharacteristicId {
        device_id,
        service_uuid: service,
        characteristic_uuid: characteristic,
    };
    adapter(&state)
        .await?
        .write(&id, &value, with_response)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn bluetooth_write(
    state: State<'_, BluetoothState>,
    device_id: String,
    service: String,
    characteristic: String,
    value: String,
) -> Result<(), String> {
    write(state, device_id, service, characteristic, value, true).await
}

#[tauri::command]
async fn bluetooth_write_without_response(
    state: State<'_, BluetoothState>,
    device_id: String,
    service: String,
    characteristic: String,
    value: String,
) -> Result<(), String> {
    write(state, device_id, service, characteristic, value, false).await
}

#[tauri::command]
async fn bluetooth_notifications(
    window: tauri::WebviewWindow,
    state: State<'_, BluetoothState>,
    device_id: String,
    service: String,
    characteristic: String,
) -> Result<(), String> {
    let id = CharacteristicId {
        device_id: device_id.clone(),
        service_uuid: service.clone(),
        characteristic_uuid: characteristic.clone(),
    };
    let mut notifications = adapter(&state)
        .await?
        .notifications(&id)
        .await
        .map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn(async move {
        while let Some(value) = notifications.next().await {
            let event = format!("notification|{device_id}|{service}|{characteristic}");
            let payload = serde_json::json!({ "event": event, "value": hex::encode(value) });
            let script = format!("window.OpenRadlrDesktop._notification({payload});");
            let _ = window.eval(&script);
        }
    });
    Ok(())
}

const DESKTOP_PLUGIN_SCRIPT: &str = r##"
  (() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    const devices = () => invoke("bluetooth_devices");
    const listeners = new Map();
    const scans = new Map();
    let nextScanId = 0;
    Object.defineProperty(window, "OpenRadlrDesktop", { value: {
      isNativePlatform: () => true,
      Plugins: { BluetoothLe: {
        initialize: () => invoke("bluetooth_initialize"),
        isEnabled: () => invoke("bluetooth_is_enabled").then(value => ({ value })),
        getDevices: async ({ deviceIds = [] } = {}) => ({ devices: (await devices()).filter(device => !deviceIds.length || deviceIds.includes(device.deviceId)) }),
        requestDevice: async (options = {}) => {
          const services = options.services || options.filters?.flatMap(filter => filter.services || []).map(String) || [];
          const { name, namePrefix } = options;
          const scanId = `scan-${++nextScanId}`;
          return new Promise((resolve, reject) => {
            const choose = device => { scans.delete(scanId); resolve(device); };
            const cancel = () => { scans.delete(scanId); reject(new Error("cancelled")); };
            scans.set(scanId, { devices: new Map(), choose, cancel });
            window.dispatchEvent(new CustomEvent("openradlrdesktopdevices", { detail: { scanId, devices: [], resolve: choose, reject: cancel, scanning: true } }));
            invoke("bluetooth_scan", { services, name, namePrefix, scanId }).catch(error => { scans.delete(scanId); reject(error); });
          });
        },
        connect: ({ deviceId }) => invoke("bluetooth_connect", { deviceId }),
        disconnect: ({ deviceId }) => invoke("bluetooth_disconnect", { deviceId }),
        getServices: async ({ deviceId }) => ({ services: await invoke("bluetooth_services", { deviceId }) }),
        write: options => invoke("bluetooth_write", options),
        writeWithoutResponse: options => invoke("bluetooth_write_without_response", options),
        addListener: (event, callback) => {
          const callbacks = listeners.get(event) || new Set();
          callbacks.add(callback); listeners.set(event, callbacks);
          return Promise.resolve({ remove: () => callbacks.delete(callback) });
        },
        startNotifications: options => invoke("bluetooth_notifications", options),
      } }
    }, configurable: false });
    Object.defineProperty(window.OpenRadlrDesktop, "_notification", { value: ({ event, value }) => {
      for (const callback of listeners.get(event) || []) callback({ value });
    }});
    Object.defineProperty(window.OpenRadlrDesktop, "_scanDevice", { value: ({ scanId, device }) => {
      const scan = scans.get(scanId); if (!scan) return;
      scan.devices.set(device.deviceId, device);
      window.dispatchEvent(new CustomEvent("openradlrdesktopdevices", { detail: { scanId, devices: [...scan.devices.values()], update: true, scanning: true } }));
    }});
    Object.defineProperty(window.OpenRadlrDesktop, "_scanFinished", { value: ({ scanId }) => {
      const scan = scans.get(scanId); if (!scan) return;
      window.dispatchEvent(new CustomEvent("openradlrdesktopdevices", { detail: { scanId, devices: [...scan.devices.values()], update: true, scanning: false } }));
    }});
  })();
"##;

fn main() {
    tauri::Builder::default()
        .manage(BluetoothState(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            bluetooth_initialize,
            bluetooth_is_enabled,
            bluetooth_devices,
            bluetooth_scan,
            bluetooth_connect,
            bluetooth_disconnect,
            bluetooth_services,
            bluetooth_write,
            bluetooth_write_without_response,
            bluetooth_notifications,
        ])
        .setup(|app| {
            let url = server_url()
                .parse()
                .expect("OPENRADLR_DESKTOP_URL must be a valid URL");
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .initialization_script(DESKTOP_PLUGIN_SCRIPT)
                .title("OpenRadlr")
                .inner_size(1280.0, 800.0)
                .min_inner_size(960.0, 600.0)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run OpenRadlr desktop");
}
