import { api } from '$tauri/api';
import type { Device, DeviceCapabilities } from '$types';

class DeviceStore {
  devices = $state<Device[]>([]);
  selected = $state<Device | null>(null);
  capabilities = $state<DeviceCapabilities | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);

  async refresh() {
    this.loading = true;
    this.error = null;
    try {
      this.devices = await api.listDevices();
      if (this.selected && !this.devices.find((d) => d.serial === this.selected!.serial)) {
        this.selected = null;
        this.capabilities = null;
      }
      if (!this.selected) {
        const firstOnline = this.devices.find((d) => d.state === 'device');
        if (firstOnline) {
          await this.select(firstOnline);
        }
      }
    } catch (e) {
      this.error = (e as Error).message;
    } finally {
      this.loading = false;
    }
  }

  async select(device: Device) {
    this.selected = device;
    this.capabilities = null;
    if (device.state !== 'device') return;
    try {
      this.capabilities = await api.probeCapabilities(device.serial);
    } catch (e) {
      this.error = (e as Error).message;
    }
  }
}

export const deviceStore = new DeviceStore();
