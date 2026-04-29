<script lang="ts">
  import { onMount } from "svelte";
  import { t, setLocale } from "@/i18n/index.svelte.ts";
  import { Button, Input, Label, Separator } from "@/components/ui";
  import { invoke } from "@tauri-apps/api/core";
  import type { AppConfig, OcrModelSet, OcrSuggestedModel } from "@/types";
  import { Download, CheckCircle2, XCircle, Loader2, RefreshCw, Zap, FolderCog, ExternalLink } from "lucide-svelte";
  import { open as openPath } from "@tauri-apps/plugin-shell";

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

  // OCR state
  let ocrSetupMode = $state<"paddle" | "custom">("paddle");
  let ocrProxy = $state("");
  let ocrBundleStatus = $state<"idle" | "downloading" | "ok" | "fail">("idle");
  let ocrBundleMsg = $state("");
  let ocrModelDir = $state("");
  let ocrGpuEnabled = $state(true);
  let ocrDetectedModels = $state<OcrModelSet[]>([]);
  let ocrSuggestedModels = $state<OcrSuggestedModel[]>([]);
  let ocrValidationStatus = $state<"idle" | "validating" | "ok" | "fail">("idle");
  let ocrDownloading = $state<string | null>(null);
  let ocrScanLoading = $state(false);
  let ocrConfigured = $state(false);
  let ocrApplyStatus = $state<"idle" | "applying" | "ok" | "fail">("idle");
  let ocrApplyMsg = $state("");

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
      if (config.ocr) {
        ocrSetupMode = "custom";
        ocrModelDir = config.ocr.modelDir;
        ocrGpuEnabled = config.ocr.gpuEnabled;
      }
      ocrConfigured = await invoke("ocr_check_configured");
      ocrModelDir = await invoke("ocr_get_model_dir");
      await scanModels(ocrModelDir);
      loadSuggestedModels();
    } catch (_) {}
  }

  async function scanModels(dir: string) {
    ocrScanLoading = true;
    try {
      ocrDetectedModels = await invoke("ocr_scan_models", { dirPath: dir });
    } catch {
      ocrDetectedModels = [];
    } finally {
      ocrScanLoading = false;
    }
  }

  async function loadSuggestedModels() {
    try {
      ocrSuggestedModels = await invoke("ocr_get_suggested_models");
    } catch {
      ocrSuggestedModels = [];
    }
  }

  async function openModelDir() {
    try {
      await openPath(ocrModelDir);
    } catch (e) {
      console.error("Failed to open directory:", e);
    }
  }

  async function rescanAndApply() {
    await scanModels(ocrModelDir);
  }

  async function applyCustomModels() {
    if (ocrDetectedModels.length === 0) return;
    ocrApplyStatus = "applying";
    try {
      await invoke("ocr_apply_local_models", {
        language: ocrDetectedModels[0].language,
        gpuEnabled: ocrGpuEnabled,
      });
      ocrApplyStatus = "ok";
      ocrApplyMsg = t("settings.ocrDownloadSuccess");
      ocrConfigured = true;
    } catch (e) {
      ocrApplyStatus = "fail";
      ocrApplyMsg = String(e);
    }
    setTimeout(() => {
      ocrApplyStatus = "idle";
      ocrApplyMsg = "";
    }, 4000);
  }

  async function downloadPaddleBundle() {
    ocrBundleStatus = "downloading";
    ocrBundleMsg = t("settings.ocrDownloadingBundle");
    try {
      await invoke("ocr_download_paddle_bundle", {
        proxy: ocrProxy || null,
      });
      ocrBundleStatus = "ok";
      ocrBundleMsg = t("settings.ocrDownloadSuccess");
      ocrConfigured = true;
      ocrGpuEnabled = true;
      ocrModelDir = await invoke("ocr_get_model_dir");
      await scanModels(ocrModelDir);
    } catch (e) {
      ocrBundleStatus = "fail";
      ocrBundleMsg = t("settings.ocrDownloadFailed");
      console.error("Bundle download failed:", e);
    }
    setTimeout(() => {
      ocrBundleStatus = "idle";
      ocrBundleMsg = "";
    }, 5000);
  }

  async function downloadAdditionalModel(model: OcrSuggestedModel) {
    ocrDownloading = model.language;
    try {
      if (model.recUrl) {
        const recPath = `${ocrModelDir}/${model.language}_PP-OCRv5_mobile_rec_infer.mnn`;
        await invoke("ocr_download_model", { url: model.recUrl, savePath: recPath, proxy: ocrProxy || null });
      }
      if (model.keysUrl) {
        const keysPath = `${ocrModelDir}/ppocr_keys_${model.language}.txt`;
        await invoke("ocr_download_model", { url: model.keysUrl, savePath: keysPath, proxy: ocrProxy || null });
      }
      await scanModels(ocrModelDir);
    } catch (e) {
      console.error("Download failed:", e);
    } finally {
      ocrDownloading = null;
    }
  }

  async function saveConfig() {
    try {
      let ocrConfig = null;
      if (ocrModelDir && ocrDetectedModels.length > 0) {
        const m = ocrDetectedModels[0];
        ocrConfig = {
          modelDir: ocrModelDir,
          detModel: m.detPath.split(/[\\/]/).pop() || "",
          recModel: m.recPath.split(/[\\/]/).pop() || "",
          keysFile: m.keysPath.split(/[\\/]/).pop() || "",
          language: m.language,
          gpuEnabled: ocrGpuEnabled,
        };
      }

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
        ocr: ocrConfig,
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

  onMount(() => {
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
            onchange={() => setLocale(language)}
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

    <!-- OCR Settings -->
    <section class="space-y-4">
      <div class="flex items-center gap-2">
        <h2 class="text-sm font-medium text-foreground">{t("settings.ocr")}</h2>
        {#if ocrConfigured}
          <CheckCircle2 size={14} class="text-green-600" />
        {/if}
      </div>
      <Separator />

      <p class="text-xs text-muted-foreground">{t("settings.ocrIntro")}</p>

      <!-- Mode tabs -->
      <div class="flex gap-2">
        <button
          class="flex-1 flex flex-col items-start gap-1 p-3 rounded-lg border transition-colors {ocrSetupMode === 'paddle' ? 'border-primary bg-primary/5' : 'border-border hover:bg-muted'}"
          onclick={() => (ocrSetupMode = "paddle")}
        >
          <div class="flex items-center gap-1.5">
            <Zap size={14} class={ocrSetupMode === 'paddle' ? 'text-primary' : 'text-muted-foreground'} />
            <span class="text-sm font-medium">{t("settings.ocrMethodPaddle")}</span>
          </div>
          <span class="text-xs text-muted-foreground">{t("settings.ocrMethodPaddleDesc")}</span>
        </button>
        <button
          class="flex-1 flex flex-col items-start gap-1 p-3 rounded-lg border transition-colors {ocrSetupMode === 'custom' ? 'border-primary bg-primary/5' : 'border-border hover:bg-muted'}"
          onclick={() => (ocrSetupMode = "custom")}
        >
          <div class="flex items-center gap-1.5">
            <FolderCog size={14} class={ocrSetupMode === 'custom' ? 'text-primary' : 'text-muted-foreground'} />
            <span class="text-sm font-medium">{t("settings.ocrMethodCustom")}</span>
          </div>
          <span class="text-xs text-muted-foreground">{t("settings.ocrMethodCustomDesc")}</span>
        </button>
      </div>

      <!-- Proxy -->
      <div class="space-y-1">
        <Label>{t("settings.ocrProxy")}</Label>
        <Input bind:value={ocrProxy} placeholder={t("settings.ocrProxyPlaceholder")} />
        <p class="text-xs text-muted-foreground">{t("settings.ocrProxyHint")}</p>
      </div>

      <!-- PaddleOCR path -->
      {#if ocrSetupMode === "paddle"}
        <div class="space-y-3 p-4 rounded-lg border border-border bg-muted/30">
          {#if ocrBundleStatus === "ok"}
            <div class="flex items-center gap-2 text-sm text-green-700">
              <CheckCircle2 size={16} />
              <span>{ocrBundleMsg}</span>
            </div>
          {:else if ocrBundleStatus === "fail"}
            <div class="flex items-center gap-2 text-sm text-destructive">
              <XCircle size={16} />
              <span>{ocrBundleMsg}</span>
            </div>
          {/if}

          <Button
            onclick={downloadPaddleBundle}
            disabled={ocrBundleStatus === "downloading"}
            class="w-full"
          >
            {#if ocrBundleStatus === "downloading"}
              <Loader2 size={14} class="animate-spin" />
              <span class="ml-2">{t("settings.ocrDownloadingBundle")}</span>
            {:else}
              <Download size={14} class="mr-1.5" />
              {t("settings.ocrOneClickDownload")}
            {/if}
          </Button>

          <!-- Additional language models -->
          {#if ocrConfigured && ocrSuggestedModels.length > 1}
            <div class="space-y-1.5 pt-2">
              <Label class="text-xs">{t("settings.ocrSuggestedModels")}</Label>
              <p class="text-xs text-muted-foreground">{t("settings.ocrSuggestedModelsHint")}</p>
              <div class="space-y-1">
                {#each ocrSuggestedModels.filter(m => m.language !== "default") as model (model.language)}
                  <div class="flex items-center justify-between px-3 py-2 rounded-lg border border-border text-sm">
                    <div>
                      <span class="text-foreground">{model.name}</span>
                      <span class="text-muted-foreground text-xs ml-2">{model.totalSize}</span>
                      <p class="text-xs text-muted-foreground">{model.description}</p>
                    </div>
                    <Button
                      variant="outline"
                      size="sm"
                      onclick={() => downloadAdditionalModel(model)}
                      disabled={ocrDownloading !== null}
                    >
                      {#if ocrDownloading === model.language}
                        <Loader2 size={14} class="animate-spin" />
                      {:else}
                        <Download size={14} class="mr-1" />
                      {/if}
                      {ocrDownloading === model.language
                        ? t("settings.ocrDownloading")
                        : t("settings.ocrDownload")}
                    </Button>
                  </div>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      {/if}

      <!-- Custom path: guide user to copy files to app directory -->
      {#if ocrSetupMode === "custom"}
        <div class="space-y-3 p-4 rounded-lg border border-border bg-muted/30">
          <!-- Step 1: Show model directory -->
          <div class="space-y-1.5">
            <Label>{t("settings.ocrModelDir")}</Label>
            <div class="flex items-center gap-2">
              <Input readonly value={ocrModelDir} class="flex-1 text-xs font-mono" />
              <Button variant="outline" size="sm" onclick={openModelDir}>
                <FolderCog size={14} class="mr-1" />{t("settings.ocrBrowse")}
              </Button>
            </div>
            <p class="text-xs text-muted-foreground">{t("settings.ocrCustomStep1")}</p>
            <ul class="list-disc list-inside text-xs text-muted-foreground space-y-0.5 ml-1">
              <li><code class="text-foreground/80 bg-muted px-1 rounded">{t("settings.ocrCustomDet")}</code></li>
              <li><code class="text-foreground/80 bg-muted px-1 rounded">{t("settings.ocrCustomRec")}</code></li>
              <li><code class="text-foreground/80 bg-muted px-1 rounded">{t("settings.ocrCustomKeys")}</code></li>
            </ul>
          </div>

          <!-- Step 2: Model download link -->
          <div class="space-y-1">
            <p class="text-xs text-muted-foreground">{t("settings.ocrCustomStep3")}</p>
            <a
              href="https://github.com/zibo-chen/rust-paddle-ocr/tree/next/models"
              target="_blank"
              rel="noopener noreferrer"
              class="inline-flex items-center gap-1 text-xs text-primary hover:underline"
            >
              github.com/zibo-chen/rust-paddle-ocr/tree/next/models
              <ExternalLink size={10} />
            </a>
            <p class="text-xs text-muted-foreground">{t("settings.ocrCustomStep2")}</p>
            <p class="text-xs text-muted-foreground">{t("settings.ocrCustomStep4")}</p>
          </div>

          <Separator />

          <!-- Step 3: Scan + detected models -->
          <div class="space-y-1.5">
            <div class="flex items-center justify-between">
              <Label>{t("settings.ocrDetectedModels")}</Label>
              <button
                class="text-xs text-muted-foreground hover:text-foreground inline-flex items-center gap-1"
                onclick={rescanAndApply}
              >
                <RefreshCw size={12} class={ocrScanLoading ? 'animate-spin' : ''} />
                Rescan
              </button>
            </div>
            {#if ocrDetectedModels.length > 0}
              <div class="space-y-1">
                {#each ocrDetectedModels as model}
                  <div class="flex items-center gap-2 px-3 py-2 rounded-lg border border-border text-sm">
                    <CheckCircle2 size={14} class="text-green-600 shrink-0" />
                    <span class="text-foreground">{model.displayName}</span>
                  </div>
                {/each}
              </div>
            {:else if !ocrScanLoading}
              <p class="text-xs text-muted-foreground">{t("settings.ocrNoModels")}</p>
            {/if}
          </div>

          <!-- Apply button -->
          {#if ocrApplyStatus === "ok"}
            <div class="flex items-center gap-2 text-sm text-green-700">
              <CheckCircle2 size={16} />
              <span>{ocrApplyMsg}</span>
            </div>
          {:else if ocrApplyStatus === "fail"}
            <div class="flex items-center gap-2 text-sm text-destructive">
              <XCircle size={16} />
              <span>{ocrApplyMsg}</span>
            </div>
          {/if}
          <Button
            onclick={applyCustomModels}
            disabled={ocrDetectedModels.length === 0 || ocrApplyStatus === "applying"}
            class="w-full"
          >
            {#if ocrApplyStatus === "applying"}
              <Loader2 size={14} class="animate-spin" />
            {:else}
              <CheckCircle2 size={14} class="mr-1.5" />
            {/if}
            {ocrApplyStatus === "applying" ? "Applying..." : "Apply Models"}
          </Button>
        </div>
      {/if}

      <!-- GPU toggle -->
      <div class="flex items-center gap-2">
        <label class="flex items-center gap-2 text-sm cursor-pointer">
          <input type="checkbox" bind:checked={ocrGpuEnabled} class="rounded" />
          <span class="text-muted-foreground">{t("settings.ocrGpuEnabled")}</span>
        </label>
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
