import { load, type Store } from "@tauri-apps/plugin-store";
import { normalizeSettings, type BatchSettings } from "./settings";

/**
 * Named setting bundles, persisted to disk by the store plugin so they survive restarts.
 * These are also how settings dialled in elsewhere (a future single-image tab) reach a
 * batch run.
 */
export class PresetStore {
  names = $state<string[]>([]);
  ready = $state(false);

  #store: Store | null = null;

  async init() {
    try {
      this.#store = await load("presets.json", { autoSave: true });
      this.names = (await this.#store.keys()).sort();
    } catch {
      // Without a store the app still works, just without persistence.
      this.names = [];
    } finally {
      this.ready = true;
    }
  }

  async get(name: string): Promise<BatchSettings | null> {
    if (!this.#store) return null;
    const stored = await this.#store.get(name);
    return stored === undefined || stored === null ? null : normalizeSettings(stored);
  }

  async save(name: string, settings: BatchSettings) {
    if (!this.#store || name.trim() === "") return;
    await this.#store.set(name.trim(), settings);
    // autoSave debounces, so a user who saves a template and immediately closes the window
    // could lose it. Flushing here means the write has landed before this resolves.
    await this.#store.save();
    if (!this.names.includes(name.trim())) {
      this.names = [...this.names, name.trim()].sort();
    }
  }

  async remove(name: string) {
    if (!this.#store) return;
    await this.#store.delete(name);
    await this.#store.save();
    this.names = this.names.filter((existing) => existing !== name);
  }
}
