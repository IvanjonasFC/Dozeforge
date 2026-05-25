import { api } from '$tauri/api';
import type { AuditReport, CpuAggregate, WakeupSources } from '$types';

class AuditStore {
  report = $state<AuditReport | null>(null);
  wakeupSources = $state<WakeupSources | null>(null);
  cpuSamples = $state<CpuAggregate[]>([]);
  running = $state(false);
  lastError = $state<string | null>(null);

  async run(serial: string) {
    this.running = true;
    this.lastError = null;
    try {
      const [report, wakeup] = await Promise.all([
        api.auditDevice(serial),
        api.listWakeupSources(serial)
      ]);
      this.report = report;
      this.wakeupSources = wakeup;
    } catch (e) {
      this.lastError = (e as Error).message;
    } finally {
      this.running = false;
    }
  }

  async sampleCpu(serial: string, durationSecs = 30) {
    this.running = true;
    this.lastError = null;
    try {
      this.cpuSamples = await api.sampleCpu(serial, durationSecs);
    } catch (e) {
      this.lastError = (e as Error).message;
    } finally {
      this.running = false;
    }
  }

  clear() {
    this.report = null;
    this.wakeupSources = null;
    this.cpuSamples = [];
  }
}

export const auditStore = new AuditStore();
