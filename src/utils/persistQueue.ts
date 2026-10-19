interface PersistEntry {
  fn: () => Promise<void>;
  resolve: () => void;
  reject: (e: unknown) => void;
}

export class PersistQueue {
  private queue: PersistEntry[] = [];
  private running = false;

  enqueue(fn: () => Promise<void>): Promise<void> {
    return new Promise((resolve, reject) => {
      this.queue.push({ fn, resolve, reject });
      void this.runNext();
    });
  }

  private async runNext() {
    if (this.running || this.queue.length === 0) return;
    this.running = true;
    const entry = this.queue.shift()!;
    try {
      await entry.fn();
      entry.resolve();
    } catch (e) {
      entry.reject(e);
    } finally {
      this.running = false;
      void this.runNext();
    }
  }
}
