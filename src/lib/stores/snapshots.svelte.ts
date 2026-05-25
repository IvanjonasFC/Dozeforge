import { api } from '$tauri/api';
import type { SnapshotMeta } from '$types';

class SnapshotStore {
  list = $state<SnapshotMeta[]>([]);
  loading = $state(false);
  lastError = $state<string | null>(null);

  async refresh() {
    this.loading = true;
    this.lastError = null;
    try {
      this.list = await api.listSnapshots();
    } catch (e) {
      this.lastError = (e as Error).message;
    } finally {
      this.loading = false;
    }
  }

  async take(serial: string, packages: string[], label?: string) {
    const meta = await api.takeSnapshot(serial, packages, label);
    await this.refresh();
    return meta;
  }

  async rollback(serial: string, id: string, only?: string[]) {
    const report = await api.rollbackSnapshot(serial, id, only);
    return report;
  }
}

export const snapshotStore = new SnapshotStore();
