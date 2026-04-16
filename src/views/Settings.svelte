<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import { Button, Input, Label, Separator } from "@/components/ui";
  import { invoke } from "@tauri-apps/api/core";
  import type { AppConfig } from "@/lib/types";

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
  let saveStatus = $state("");
  let connectionStatus = $state<"idle" | "testing" | "ok" | "fail">("idle");

  async function loadConfig() {
    try {
      const config: AppConfig = await invoke("get_config");
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
      }
    } catch (_) {}
  }

  async function saveConfig() {
    try {
      const config: AppConfig = {
        general: {
          language,
          theme,
          default_export_dir: null,
          recent_files_max: 20,
        },
        s3: s3Enabled
          ? {
              auth_mode: s3AuthMode,
              endpoint: s3Endpoint,
              region: s3Region,
              bucket: s3Bucket,
              access_key: s3AccessKey,
              secret_key: s3SecretKey,
              session_token: s3SessionToken || null,
              force_path_style: s3PathStyle,
              root_prefix: s3RootPrefix || null,
              max_versions: s3MaxVersions ? parseInt(s3MaxVersions) : null,
              version_ttl_days: s3VersionTtl ? parseInt(s3VersionTtl) : null,
            }
          : null,
      };
      await invoke("update_config", { newConfig: config });
      saveStatus = t("settings.saved");
      setTimeout(() => (saveStatus = ""), 2000);
    } catch (e) {
      saveStatus = String(e);
      setTimeout(() => (saveStatus = ""), 3000);
    }
  }

  async function testConnection() {
    if (!s3Enabled || !s3Endpoint || !s3Bucket) return;
    connectionStatus = "testing";
    try {
      await invoke("s3_test_connection", {
        s3Config: {
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
        },
      });
      connectionStatus = "ok";
    } catch {
      connectionStatus = "fail";
    }
    setTimeout(() => (connectionStatus = "idle"), 4000);
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
