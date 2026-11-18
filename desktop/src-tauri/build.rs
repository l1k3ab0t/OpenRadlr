fn main() {
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "bluetooth_initialize",
            "bluetooth_is_enabled",
            "bluetooth_devices",
            "bluetooth_scan",
            "bluetooth_connect",
            "bluetooth_disconnect",
            "bluetooth_services",
            "bluetooth_write",
            "bluetooth_write_without_response",
            "bluetooth_notifications",
        ]));
    tauri_build::try_build(attributes).expect("failed to generate Tauri permissions");
}
