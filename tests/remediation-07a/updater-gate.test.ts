import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const checkMock = vi.fn();
const askMock = vi.fn();
const relaunchMock = vi.fn();
const downloadAndInstall = vi.fn();
const updateClose = vi.fn();
const invokeMock = vi.fn();

vi.mock("@tauri-apps/plugin-updater", () => ({
  check: () => checkMock(),
}));
vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: () => relaunchMock(),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  ask: (...args: unknown[]) => askMock(...args),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));
vi.mock("@/i18n/index.svelte.ts", () => ({
  t: (key: string) => key,
}));

import { get } from "svelte/store";
import { checkForUpdates, updaterState } from "@/updater";
import {
  beginFileWrite,
  endFileWrite,
} from "@/document/session.svelte.ts";

function fakeUpdate() {
  return {
    version: "9.9.9",
    body: null,
    downloadAndInstall: (...args: unknown[]) => downloadAndInstall(...args),
    close: () => updateClose(),
  };
}

describe("update/restart coordination with in-flight writes (07-A)", () => {
  beforeEach(() => {
    vi.stubEnv("PROD", true);
    checkMock.mockReset();
    askMock.mockReset();
    relaunchMock.mockReset();
    downloadAndInstall.mockReset();
    updateClose.mockReset();
  });

  afterEach(() => {
    vi.unstubAllEnvs();
    endFileWrite("/upd.pdf");
  });

  it("refuses to download and install while a write is in flight", async () => {
    checkMock.mockResolvedValue(fakeUpdate());
    askMock.mockResolvedValue(true); // confirms the install dialog
    beginFileWrite("/upd.pdf");

    const result = await checkForUpdates();

    expect(result).toBe("error");
    expect(downloadAndInstall).not.toHaveBeenCalled();
    expect(get(updaterState).status).toBe("error");
    expect(get(updaterState).error).toBe("updater.busyEditing");
    expect(updateClose).toHaveBeenCalled();
    endFileWrite("/upd.pdf");
  });

  it("downloads when idle but refuses to relaunch if a write started meanwhile", async () => {
    checkMock.mockResolvedValue(fakeUpdate());
    askMock.mockResolvedValue(true);
    downloadAndInstall.mockImplementation(async () => {
      // Simulate an edit starting while the update downloads.
      beginFileWrite("/upd.pdf");
    });

    const result = await checkForUpdates();

    expect(downloadAndInstall).toHaveBeenCalledOnce();
    expect(relaunchMock).not.toHaveBeenCalled();
    expect(result).toBe("error");
    expect(get(updaterState).error).toBe("updater.busyEditing");
  });

  it("relaunches when no write is in flight", async () => {
    checkMock.mockResolvedValue(fakeUpdate());
    askMock.mockResolvedValue(true);
    downloadAndInstall.mockResolvedValue(undefined);
    relaunchMock.mockResolvedValue(undefined);

    const result = await checkForUpdates();

    expect(downloadAndInstall).toHaveBeenCalledOnce();
    expect(relaunchMock).toHaveBeenCalledOnce();
    expect(result).toBe("installed");
    expect(get(updaterState).status).toBe("installed");
  });
});
