import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { ask } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { writable } from "svelte/store";
import { t } from "@/i18n/index.svelte.ts";

export type UpdateStatus =
  | "up-to-date"
  | "dev-skip"
  | "declined"
  | "installed"
  | "error";

export interface UpdaterState {
  status:
    | "idle"
    | "checking"
    | "up-to-date"
    | "available"
    | "downloading"
    | "installing"
    | "declined"
    | "installed"
    | "dev-skip"
    | "error";
  error: string;
  downloadedBytes: number;
  totalBytes: number | null;
}

export const updaterState = writable<UpdaterState>({
  status: "idle",
  error: "",
  downloadedBytes: 0,
  totalBytes: null,
});

let inFlightCheck: Promise<UpdateStatus> | null = null;

function setStatus(status: UpdaterState["status"], error = "") {
  updaterState.set({
    status,
    error,
    downloadedBytes: 0,
    totalBytes: null,
  });
}

function updateDownloadProgress(event: DownloadEvent) {
  updaterState.update((current) => {
    if (event.event === "Started") {
      return {
        ...current,
        status: "downloading",
        downloadedBytes: 0,
        totalBytes: event.data.contentLength ?? null,
      };
    }
    if (event.event === "Progress") {
      return {
        ...current,
        downloadedBytes: current.downloadedBytes + event.data.chunkLength,
      };
    }
    return current;
  });
}

async function checkForUpdatesOnce(): Promise<UpdateStatus> {
  if (!import.meta.env.PROD) {
    setStatus("dev-skip");
    return "dev-skip";
  }

  setStatus("checking");
  let update: Update | null = null;
  let result: UpdateStatus = "error";
  try {
    update = await check();
    if (!update) {
      setStatus("up-to-date");
      result = "up-to-date";
    } else {
      setStatus("available");
      const body = update.body ? `\n\n${update.body}` : "";
      const confirmed = await ask(
        `${t("updater.available")}: ${update.version}${body}`,
        { title: t("updater.title"), kind: "info" },
      );
      if (!confirmed) {
        setStatus("declined");
        result = "declined";
      } else {
        setStatus("downloading");
        await update.downloadAndInstall(updateDownloadProgress);
        setStatus("installing");
        const restart = await ask(t("updater.restartPrompt"), {
          title: t("updater.title"),
          kind: "info",
        });
        if (restart) await relaunch();
        setStatus("installed");
        result = "installed";
      }
    }
  } catch (error) {
    setStatus("error", String(error));
    result = "error";
  } finally {
    if (update) {
      try {
        await update.close();
      } catch (error) {
        setStatus("error", String(error));
        result = "error";
      }
    }
  }
  return result;
}

export function checkForUpdates(): Promise<UpdateStatus> {
  if (inFlightCheck) return inFlightCheck;
  const task = checkForUpdatesOnce();
  const sharedTask = task.finally(() => {
    if (inFlightCheck === sharedTask) inFlightCheck = null;
  });
  inFlightCheck = sharedTask;
  return sharedTask;
}

/** Startup auto-check (silent on failure), gated by the user setting. */
export async function startupUpdateCheck() {
  try {
    const cfg = await invoke<{ general: { auto_update_check: boolean } }>(
      "get_config",
    );
    if (cfg.general.auto_update_check) await checkForUpdates();
  } catch {
    // Startup checks must never block the application.
  }
}
