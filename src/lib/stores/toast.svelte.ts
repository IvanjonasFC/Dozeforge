/**
 * Lightweight toast notifications — non-intrusive feedback that matches the app
 * theme. Replaces native `alert()` for success/error/info messages. Destructive
 * confirmations still use `confirm()` (a toast can't block an action).
 *
 * Usage:  toast.success('Done'); toast.error(msg); toast.info('Tip…');
 */

export type ToastKind = 'success' | 'error' | 'info';

export interface Toast {
  id: number;
  kind: ToastKind;
  message: string;
}

class ToastStore {
  items = $state<Toast[]>([]);
  private nextId = 1;

  private push(kind: ToastKind, message: string, ms: number): number {
    const id = this.nextId++;
    this.items.push({ id, kind, message });
    if (ms > 0) setTimeout(() => this.dismiss(id), ms);
    return id;
  }

  /** Positive feedback (auto-dismiss ~3.5s). */
  success(message: string, ms = 3500): number {
    return this.push('success', message, ms);
  }

  /** Errors linger a bit longer so they can be read. */
  error(message: string, ms = 6000): number {
    return this.push('error', message, ms);
  }

  /** Neutral tips / info. */
  info(message: string, ms = 4500): number {
    return this.push('info', message, ms);
  }

  dismiss(id: number): void {
    this.items = this.items.filter((t) => t.id !== id);
  }
}

export const toast = new ToastStore();
