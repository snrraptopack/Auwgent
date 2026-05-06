export const db = {
  async load<T>(path: string, fallback?: T): Promise<T> {
    const file = Bun.file(path);

    if (!(await file.exists())) {
      return fallback !== undefined ? fallback : ({} as T);
    }

    try {
      return await file.json();
    } catch {
      // Corrupted JSON — return fallback and optionally log
      return fallback !== undefined ? fallback : ({} as T);
    }
  },

  async save(path: string, data: unknown) {
    await Bun.write(path, JSON.stringify(data, null, 2));
  }
};
