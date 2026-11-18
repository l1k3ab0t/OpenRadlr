# OpenRadlr desktop

The Linux desktop client is a Tauri shell for the normal OpenRadlr page with a
native BlueZ Bluetooth bridge for trainers, heart-rate monitors and controllers.

## Development

On Debian or Ubuntu, install the Tauri prerequisites:

```bash
sudo apt install build-essential curl file libayatana-appindicator3-dev libssl-dev \
  libwebkit2gtk-4.1-dev libxdo-dev librsvg2-dev
```

Then run:

```bash
npm install
npm run dev
```

The default server is `https://app.openradlr.com`. Build against a local or
self-hosted server with:

```bash
OPENRADLR_DESKTOP_URL=http://127.0.0.1:8080 npm run dev
```

The setting is compiled into the host. Rebuild the desktop app after changing
it.

`npm run build -- --no-bundle` writes the release binary to
`src-tauri/target/release/openradlr-desktop`.
