import { api } from '$tauri/api';
import type { Device, DeviceCapabilities } from '$types';

class DeviceStore {
  devices = $state<Device[]>([]);
  mdnsServices = $state<{address: string, service_type: string}[]>([]);
  selected = $state<Device | null>(null);
  capabilities = $state<DeviceCapabilities | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);
  
  rootMode = $state(false);
  hasRootAccess = $state(false);

  batteryLevel = $state<number | null>(null);
  batteryStatus = $state<string | null>(null);

  private pingTimer: number | null = null;

  async refresh() {
    this.loading = true;
    this.error = null;
    try {
      // Fetch USB/TCP connected devices immediately
      const devs = await api.listDevices();
      this.devices = devs;
      
      if (this.selected && !this.devices.find((d) => d.serial === this.selected!.serial)) {
        this.selected = null;
        this.capabilities = null;
        this.batteryLevel = null;
        this.batteryStatus = null;
      }
      if (!this.selected) {
        const firstOnline = this.devices.find((d) => d.state === 'device');
        if (firstOnline) {
          await this.select(firstOnline);
        }
      }

      // Fetch mDNS services in background (takes up to 10 seconds)
      api.adbMdnsServices().then(mdns => {
        this.mdnsServices = mdns;
      }).catch(() => {
        this.mdnsServices = [];
      });
    } catch (e) {
      this.error = (e as Error).message;
    } finally {
      this.loading = false;
    }
  }

  async select(device: Device) {
    this.selected = device;
    this.capabilities = null;
    this.hasRootAccess = false;
    
    if (device.state !== 'device') return;
    try {
      this.capabilities = await api.probeCapabilities(device.serial);
      // Automatically check if the device has root access (user must grant prompt on phone)
      this.hasRootAccess = await api.checkRoot(device.serial);
      if (!this.hasRootAccess) {
        this.rootMode = false;
      }
    } catch (e) {
      this.error = (e as Error).message;
    }

    if (this.pingTimer) clearInterval(this.pingTimer);
    this.batteryLevel = null;
    this.batteryStatus = null;
    if (this.selected && this.selected.state === 'device') {
      this.updateBattery();
      this.pingTimer = window.setInterval(() => this.updateBattery(), 5000);
    }
  }

  // Heartbeat: keeps the alive-check and feeds the topbar battery gauge.
  private async updateBattery() {
    if (!this.selected) return;
    try {
      const b = await api.batteryHealth(this.selected.serial);
      this.batteryLevel = b.level_percent;
      this.batteryStatus = b.status;
    } catch (e) {
      console.warn('Heartbeat ping failed, refreshing devices...');
      await this.refresh();
    }
  }

  toggleRootMode() {
    if (this.hasRootAccess) {
      this.rootMode = !this.rootMode;
    } else {
      alert('This device does not have Root access granted or is not rooted.');
    }
  }
}

export const deviceStore = new DeviceStore();
