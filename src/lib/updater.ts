import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { ask } from "@tauri-apps/plugin-dialog";
import { t } from "@/i18n/index.svelte.ts";

export type UpdateStatus =
  | "up-to-date"
  | "dev-skip"
  | "declined"
  | "installed"
  | "error";

async function installUpdate(update: Update): Promise<boolean> {
  await update.downloadAndInstall();
  const restart = await ask(t("updater.restartPrompt"), {
    title: t("updater.title"),
    kind: "info",
  });
  if (restart) {
    await relaunch();
  }
  return true;
}

/**
 * Check GitHub Releases for a newer version. In dev builds this is a no-op.
 * When an update is found the user is asked to download+install; on success
 * they are offered a relaunch.
 */
export async function checkForUpdates(): Promise<UpdateStatus> {
  if (!import.meta.env.PROD) {
    return "dev-skip";
  }
  let update: Update | null = null;
  try {
    update = await check();
  } catch {
    return "error";
  }
  if (!update) {
    return "up-to-date";
  }

  const body = update.body ? `\n\n${update.body}` : "";
  const confirmed = await ask(
    `${t("updater.available")}: ${update.version}${body}`,
    { title: t("updater.title"), kind: "info" },
  );
  if (!confirmed) {
    return "declined";
  }
  try {
    await installUpdate(update);
    return "installed";
  } catch {
    return "error";
  }
}

/** Startup auto-check (silent on failure), gated by the user setting. */
export async function startupUpdateCheck() {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const cfg = await invoke<{ general: { auto_update_check: boolean } }>(
      "get_config",
    );
    if (cfg.general.auto_update_check) {
      await checkForUpdates();
    }
  } catch {
    // silent — updater must never block startup
  }
}
