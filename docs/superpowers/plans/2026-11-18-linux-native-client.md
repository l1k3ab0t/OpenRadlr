# Linux native client plan

1. Add the Tauri Linux shell with a build-time server URL and no exposed native
   commands.
2. Add a CI job that compiles the desktop crate on Ubuntu after the system
   dependencies are defined.
3. Implement the BlueZ GATT adapter behind a Rust trait and test it with a
   fake adapter. The implementation lives in `desktop/bluetooth/`.
4. Define a narrow desktop plugin bridge and make `native/app.js` install the
   existing `NativeBluetooth` polyfill from it.
5. Add device-level checks with a real trainer, heart-rate strap and controller.
6. Add a Flatpak manifest, portal/file-save integration and release automation.
