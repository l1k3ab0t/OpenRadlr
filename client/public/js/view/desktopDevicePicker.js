import { t } from "../i18n/i18n.js";

/** Installs the Linux desktop chooser in the same document and visual language as device setup. */
export function installDesktopDevicePicker() {
  const dialog = document.getElementById("desktop-device-picker");
  if (!dialog) return;
  const list = document.getElementById("desktop-device-picker-list");
  const status = document.getElementById("desktop-device-picker-status");
  const cancel = document.getElementById("desktop-device-picker-cancel");
  let pending = null;

  const close = (device = null) => {
    if (!pending) return;
    const { resolve, reject } = pending;
    pending = null;
    dialog.close();
    if (device) resolve(device);
    else reject(new Error("cancelled"));
  };

  const render = ({ devices, scanning }) => {
    list.replaceChildren(...devices
      .sort((a, b) => (a.name ?? "").localeCompare(b.name ?? ""))
      .map((device) => deviceButton(device, () => close(device))));
    status.textContent = scanning ? t("desktopBluetooth.scanning") : devices.length ? "" : t("desktopBluetooth.none");
  };

  cancel.addEventListener("click", () => close());
  dialog.addEventListener("cancel", (event) => {
    event.preventDefault();
    close();
  });
  window.addEventListener("openradlrdesktopdevices", ({ detail }) => {
    if (detail.update) {
      if (!pending || detail.scanId !== pending.scanId) return;
      render(detail);
      return;
    }
    if (pending) close();
    pending = detail;
    render(detail);
    dialog.showModal();
    cancel.focus();
  });
}

function deviceButton(device, choose) {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "desktop-device-choice";
  button.append(Object.assign(document.createElement("strong"), { textContent: device.name || t("desktopBluetooth.unnamed") }));
  button.append(Object.assign(document.createElement("span"), { textContent: device.deviceId }));
  button.addEventListener("click", choose);
  return button;
}
