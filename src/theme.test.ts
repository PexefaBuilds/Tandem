import { describe, expect, it, vi } from "vitest";

describe("theme", () => {
  it("falls back to system for unknown values", async () => {
    const store: Record<string, string> = { "tandem.theme": "purple" };
    vi.stubGlobal("localStorage", { getItem: (k: string) => store[k] ?? null, setItem: () => {} });
    const { savedTheme } = await import("./theme");
    expect(savedTheme()).toBe("system");
  });
});
