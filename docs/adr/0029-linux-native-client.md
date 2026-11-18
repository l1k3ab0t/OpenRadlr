# 29. A Linux-native client shell

Date: 2026-11-18 · Status: accepted

## Context

The browser client is the product UI, but Web Bluetooth is not consistently
available in Linux web views. Linux riders also expect a desktop package that
integrates with the system and reaches BlueZ directly.

Rewriting the menu, ride UI and low-poly renderer in GTK would duplicate a
large, actively-developed client. The protocol and the existing device layer
are deliberately independent from the browser composition root.

## Decision

- Add a Linux-only [Tauri 2](https://v2.tauri.app/) shell in `desktop/`. It
  opens the normal OpenRadlr page from `https://app.openradlr.com` by default,
  as the mobile shell does. `OPENRADLR_DESKTOP_URL` selects a server when the
  shell is built.
- Use a Rust BlueZ adapter with `bluer`, exposed through the small plugin
  contract consumed by `client/public/js/ble/nativeBluetooth.js`. The existing
  FTMS, Cycling Power, heart-rate and controller classes remain shared
  JavaScript.
- Restrict Tauri IPC to the trusted OpenRadlr origins. The native bridge owns
  BlueZ discovery and GATT operations; the client owns the device-picker UI.
- Distribute the finished application primarily as a Flatpak. Its manifest
  will declare the BlueZ D-Bus access it needs. An AppImage is optional after
  the Flatpak works.

## Consequences

- The client stays in one UI and rendering implementation. A server deploy
  updates the page used by the desktop client without a desktop release.
- Linux Bluetooth works independently of browser Web Bluetooth support, while
  the page and connection UI remain shared with other platforms.
- Loading an arbitrary server must remain a build-time choice until native
  Bluetooth access has an explicit per-server trust model.
- Tauri requires the usual Linux WebKitGTK and desktop build dependencies to
  build from source.
