<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import { Button, Input, Label, Separator } from "@/components/ui";
  import { invoke } from "@tauri-apps/api/core";
  import { ask } from "@tauri-apps/plugin-dialog";
  import type { AppConfig, S3Config } from "@/lib/types";

  let language = $state("zh");
  let theme = $state("system");
  let s3Enabled = $state(false);
  let s3AuthMode = $state<"none" | "static" | "env">("static");
  let s3Endpoint = $state("");
  let s3Region = $state("");
  let s3Bucket = $state("");
  let s3AccessKey = $state("");
  let s3SecretKey = $state("");
  let s3SessionToken = $state("");
  let s3PathStyle = $state(false);
  let s3RootPrefix = $state("");
  let s3MaxVersions = $state("");
  let s3VersionTtl = $state("");
  let s3AutoBackup = $state(false);
  let saveStatus = $state("");
  let connectionStatus = $state<"idle" | "testing" | "ok" | "fail">("idle");
  let loadedConfig = $state<AppConfig | null>(null);

  // Cloud sync state
  let syncing = $state<"idle" | "backup" | "versions" | "restore">("idle");
  let syncMsg = $state("");
  let syncMsgIsError = $state(false);
  let backupVersions = $state<
    { version_id: string; size: number; last_modified: string; is_latest: boolean }[]
  >([]);
  let showVersions = $state(false);

  function buildS3Config(): S3Config {
    return {
      auth_mode: s3AuthMode,
      endpoint: s3Endpoint,
      region: s3Region || "us-east-1",
      bucket: s3Bucket,
      access_key: s3AccessKey,
      secret_key: s3SecretKey,
      session_token: s3SessionToken || null,
      force_path_style: s3PathStyle,
      root_prefix: s3RootPrefix || null,
      max_versions: s3MaxVersions ? parseInt(s3MaxVersions) : null,
      version_ttl_days: s3VersionTtl ? parseInt(s3VersionTtl) : null,
      auto_backup_config: s3AutoBackup,
      last_backup_at: loadedConfig?.s3?.last_backup_at ?? null,
    };
  }

  function flashSync(msg: string, isError = false) {
    syncMsg = msg;
    syncMsgIsError = isError;
    setTimeout(() => (syncMsg = ""), isError ? 5000 : 3000);
  }

  function formatEpoch(sec: string | null | undefined): string {
    if (!sec) return "";
    const d = new Date(parseInt(sec) * 1000);
    return isNaN(d.getTime()) ? "" : d.toLocaleString();
  }

  function formatS3Time(iso: string): string {
    const d = new Date(iso);
    return isNaN(d.getTime()) ? iso : d.toLocaleString();
  }

  async function loadConfig() {
    try {
      const config: AppConfig = await invoke("get_config");
      loadedConfig = config;
      language = config.general.language;
      theme = config.general.theme;
      if (config.s3) {
        s3Enabled = true;
        s3AuthMode = config.s3.auth_mode || "static";
        s3Endpoint = config.s3.endpoint;
        s3Region = config.s3.region;
        s3Bucket = config.s3.bucket;
        s3AccessKey = config.s3.access_key;
        s3SecretKey = config.s3.secret_key;
        s3SessionToken = config.s3.session_token || "";
        s3PathStyle = config.s3.force_path_style;
        s3RootPrefix = config.s3.root_prefix || "";
        s3MaxVersions = config.s3.max_versions?.toString() || "";
        s3VersionTtl = config.s3.version_ttl_days?.toString() || "";
        s3AutoBackup = config.s3.auto_backup_config ?? false;
      }
    } catch (_) {}
  }

  async function saveConfig() {
    try {
      const config: AppConfig = {
        general: {
          language,
          theme,
          default_export_dir: loadedConfig?.general.default_export_dir ?? null,
          recent_files_max: loadedConfig?.general.recent_files_max ?? 20,
          recent_files: loadedConfig?.general.recent_files ?? [],
        },
        s3: s3Enabled ? buildS3Config() : null,
      };
      await invoke("update_config", { newConfig: config });
      loadedConfig = config;
      saveStatus = t("settings.saved");
      setTimeout(() => (saveStatus = ""), 2000);
      if (s3Enabled && s3AutoBackup) {
        backupNow(true);
      }
    } catch (e) {
      saveStatus = String(e);
      setTimeout(() => (saveStatus = ""), 3000);
    }
  }

  async function testConnection() {
    if (!s3Enabled || !s3Endpoint || !s3Bucket) return;
    connectionStatus = "testing";
    try {
      await invoke("s3_test_connection", { s3Config: buildS3Config() });
      connectionStatus = "ok";
    } catch {
      connectionStatus = "fail";
    }
    setTimeout(() => (connectionStatus = "idle"), 4000);
  }

  // ─── Cloud sync actions ────────────────────────────────────────────

  async function backupNow(auto = false) {
    if (!s3Enabled || !s3Bucket) return;
    syncing = "backup";
    try {
      const info = await invoke<{ backed_at: string }>("sync_backup_config", {
        s3Config: buildS3Config(),
      });
      if (loadedConfig?.s3) loadedConfig.s3.last_backup_at = info.backed_at;
      flashSync(t("settings.syncBackupDone"));
    } catch (e) {
      flashSync(String(e), true);
    } finally {
      syncing = "idle";
    }
  }

  async function toggleVersions() {
    showVersions = !showVersions;
    if (showVersions) {
      syncing = "versions";
      backupVersions = [];
      try {
        const versions = await invoke<
          { version_id: string; size: number; last_modified: string; is_latest: boolean }[]
        >("sync_list_backup_versions", { s3Config: buildS3Config() });
        backupVersions = versions;
      } catch (e) {
        flashSync(String(e), true);
        showVersions = false;
      } finally {
        syncing = "idle";
      }
    }
  }

  async function restoreVersion(versionId: string) {
    const confirmed = await ask(t("settings.syncRestoreConfirm"), {
      title: t("settings.syncRestore"),
      kind: "warning",
    });
    if (!confirmed) return;
    syncing = "restore";
    try {
      const restored: AppConfig = await invoke("sync_restore_config", {
        s3Config: buildS3Config(),
        versionId: versionId || null,
      });
      loadedConfig = restored;
      s3Enabled = true;
      await loadConfig();
      flashSync(t("settings.syncRestoreDone"));
    } catch (e) {
      flashSync(String(e), true);
    } finally {
      syncing = "idle";
    }
  }

  $effect(() => {
    loadConfig();
  });
</script>

<div class="flex flex-col h-full overflow-auto p-6">
  <h1 class="text-xl font-semibold text-foreground mb-6">{t("settings.title")}</h1>

  <div class="max-w-2xl space-y-8">
    <!-- General Settings -->
    <section class="space-y-4">
      <h2 class="text-sm font-medium text-foreground">{t("settings.general")}</h2>
      <Separator />

      <div class="grid grid-cols-2 gap-4">
        <div class="space-y-2">
          <Label>{t("settings.language")}</Label>
          <select
            bind:value={language}
            class="flex h-10 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
          >
            <option value="zh">中文</option>
            <option value="en">English</option>
          </select>
        </div>
        <div class="space-y-2">
          <Label>{t("settings.theme")}</Label>
          <select
            bind:value={theme}
            class="flex h-10 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
          >
            <option value="system">{t("settings.themeSystem")}</option>
            <option value="light">{t("settings.themeLight")}</option>
            <option value="dark">{t("settings.themeDark")}</option>
          </select>
        </div>
      </div>
    </section>

    <!-- S3 Settings -->
    <section class="space-y-4">
      <div class="flex items-center justify-between">
        <h2 class="text-sm font-medium text-foreground">{t("settings.s3")}</h2>
        <label class="flex items-center gap-2 text-sm">
          <input type="checkbox" bind:checked={s3Enabled} class="rounded" />
          <span class="text-muted-foreground">{s3Enabled ? "Enabled" : "Disabled"}</span>
        </label>
      </div>
      <Separator />

      {#if s3Enabled}
        <div class="space-y-3">
          <div class="space-y-1.5">
            <Label>{t("settings.s3AuthMode")}</Label>
            <select
              bind:value={s3AuthMode}
              class="flex h-10 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
            >
              <option value="none">{t("settings.s3AuthNone")}</option>
              <option value="static">{t("settings.s3AuthStatic")}</option>
              <option value="env">{t("settings.s3AuthEnv")}</option>
            </select>
          </div>
          <div class="grid grid-cols-2 gap-3">
            <div class="space-y-1.5">
              <Label>{t("settings.s3Endpoint")}</Label>
              <Input bind:value={s3Endpoint} placeholder="https://s3.amazonaws.com" />
            </div>
            <div class="space-y-1.5">
              <Label>{t("settings.s3Region")}</Label>
              <Input bind:value={s3Region} placeholder="us-east-1" />
            </div>
          </div>
          <div class="space-y-1.5">
            <Label>{t("settings.s3Bucket")}</Label>
            <Input bind:value={s3Bucket} placeholder="my-bucket" />
          </div>
          {#if s3AuthMode === "static"}
            <div class="grid grid-cols-2 gap-3">
              <div class="space-y-1.5">
                <Label>{t("settings.s3AccessKey")}</Label>
                <Input type="password" bind:value={s3AccessKey} />
              </div>
              <div class="space-y-1.5">
                <Label>{t("settings.s3SecretKey")}</Label>
                <Input type="password" bind:value={s3SecretKey} />
              </div>
            </div>
            <div class="space-y-1.5">
              <Label>{t("settings.s3SessionToken")}</Label>
              <Input bind:value={s3SessionToken} />
            </div>
          {:else if s3AuthMode === "env"}
            <p class="text-xs text-muted-foreground">
              {t("settings.s3AuthEnvHint")}
            </p>
          {/if}
          <div class="grid grid-cols-3 gap-3">
            <div class="space-y-1.5">
              <Label>{t("settings.s3RootPrefix")}</Label>
              <Input bind:value={s3RootPrefix} placeholder="pdf/" />
            </div>
            <div class="space-y-1.5">
              <Label>{t("settings.s3MaxVersions")}</Label>
              <Input type="number" bind:value={s3MaxVersions} placeholder="10" />
            </div>
            <div class="space-y-1.5">
              <Label>{t("settings.s3VersionTtl")}</Label>
              <Input type="number" bind:value={s3VersionTtl} placeholder="90" />
            </div>
          </div>
          <div class="flex items-center gap-2">
            <label class="flex items-center gap-2 text-sm">
              <input type="checkbox" bind:checked={s3PathStyle} class="rounded" />
              <span class="text-muted-foreground">{t("settings.s3PathStyle")}</span>
            </label>
          </div>
          <div class="flex items-center gap-3">
            <Button
              variant="outline"
              size="sm"
              onclick={testConnection}
              disabled={connectionStatus === "testing"}
            >
              {connectionStatus === "testing"
                ? "Testing..."
                : t("settings.s3TestConnection")}
            </Button>
            {#if connectionStatus === "ok"}
              <span class="text-sm text-green-600">{t("settings.s3Connected")}</span>
            {:else if connectionStatus === "fail"}
              <span class="text-sm text-destructive">{t("settings.s3Failed")}</span>
            {/if}
          </div>

          <!-- Cloud sync (config backup / restore) -->
          <div class="rounded-lg border border-border p-3 space-y-3">
            <div>
              <p class="text-sm font-medium">{t("settings.syncTitle")}</p>
              <p class="text-xs text-muted-foreground mt-0.5">{t("settings.syncHint")}</p>
            </div>
            <label class="flex items-center gap-2 text-sm">
              <input type="checkbox" bind:checked={s3AutoBackup} class="rounded" />
              <span class="text-muted-foreground">{t("settings.syncAuto")}</span>
            </label>
            {#if loadedConfig?.s3?.last_backup_at}
              <p class="text-xs text-muted-foreground">
                {t("settings.syncLastBackup")}: {formatEpoch(loadedConfig.s3.last_backup_at)}
              </p>
            {/if}
            <div class="flex items-center gap-2 flex-wrap">
              <Button
                variant="outline"
                size="sm"
                onclick={() => backupNow()}
                disabled={syncing !== "idle"}
              >
                {t("settings.syncBackupNow")}
              </Button>
              <Button
                variant="outline"
                size="sm"
                onclick={toggleVersions}
                disabled={syncing !== "idle"}
              >
                {showVersions
                  ? t("settings.syncHideVersions")
                  : t("settings.syncShowVersions")}
              </Button>
              {#if syncing !== "idle"}
                <span class="text-xs text-muted-foreground">{t("app.loading")}</span>
              {/if}
              {#if syncMsg}
                <span
                  class="text-xs {syncMsgIsError
                    ? 'text-destructive'
                    : 'text-green-600'} break-all"
                >
                  {syncMsg}
                </span>
              {/if}
            </div>
            {#if showVersions}
              <div class="space-y-1 max-h-48 overflow-auto">
                {#if backupVersions.length === 0}
                  <p class="text-xs text-muted-foreground">{t("settings.syncNoVersions")}</p>
                {:else}
                  {#each backupVersions as ver (ver.version_id)}
                    <div class="flex items-center justify-between gap-2 px-2 py-1.5 rounded border border-border text-xs">
                      <div class="flex items-center gap-2 min-w-0">
                        {#if ver.is_latest}
                          <span class="px-1.5 py-0.5 rounded bg-primary/10 text-primary text-[10px] shrink-0">
                            {t("settings.syncLatest")}
                          </span>
                        {/if}
                        <span class="text-muted-foreground truncate">{formatS3Time(ver.last_modified)}</span>
                        <span class="text-muted-foreground/60 shrink-0">{ver.size} B</span>
                      </div>
                      <button
                        class="text-[11px] text-muted-foreground hover:text-foreground shrink-0 disabled:opacity-50"
                        disabled={syncing !== "idle"}
                        onclick={() => restoreVersion(ver.version_id)}
                      >
                        {t("settings.syncRestore")}
                      </button>
                    </div>
                  {/each}
                {/if}
              </div>
            {/if}
          </div>
        </div>
      {/if}
    </section>

    <!-- Save button -->
    <div class="flex items-center gap-3 pt-2">
      <Button onclick={saveConfig}>{t("settings.save")}</Button>
      {#if saveStatus}
        <span
          class="text-sm {saveStatus === t('settings.saved')
            ? 'text-green-600'
            : 'text-destructive'}"
        >
          {saveStatus}
        </span>
      {/if}
    </div>
  </div>
</div>
