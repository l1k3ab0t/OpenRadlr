# Linux native client design

## Goal

Offer OpenRadlr as a Linux desktop application with the same user interface
and server protocol as the browser client, while eventually connecting trainers
through BlueZ without depending on Web Bluetooth.

## First deliverable

`desktop/` contains a Tauri 2 application for Linux. It creates a desktop
window for the configured OpenRadlr server; the production default is
`https://app.openradlr.com`. It does not provide native page APIs yet, so a
remote page cannot invoke Rust commands or access host resources.

The configured address is set when the application is built with
`OPENRADLR_DESKTOP_URL`. This matches the mobile app's model: the page and the
server always come from the same deployment, preserving its WebSocket URL,
same-origin API requests, cookie session and Strava flow.

## Native Bluetooth design

The later BlueZ layer uses Rust and `bluer`. It exposes the exact asynchronous
operations expected by `NativeBluetooth`: initialise, scan/select a device,
retrieve remembered devices, connect/disconnect, discover GATT services and
characteristics, subscribe, and write with or without response. Values use the
existing hex-string bridge format.

The page gets that capability only through an explicit desktop runtime marker.
The browser path stays untouched. The Rust host must not expose generic Tauri
IPC to a remotely loaded page.

## Packaging

Flatpak is the release format. The manifest will request the minimal BlueZ
D-Bus permissions after the adapter is usable. Development builds remain
ordinary Tauri builds so developers can run against a local server.

## Non-goals

- A GTK rewrite of the UI or 3D renderer.
- A new game protocol or server endpoint.
- Native BLE in the initial shell.
- Runtime switching to untrusted servers.
