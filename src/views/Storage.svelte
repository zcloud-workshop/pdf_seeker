<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import { invoke } from "@tauri-apps/api/core";
  import { ask, open, save } from "@tauri-apps/plugin-dialog";
  import { tick } from "svelte";
  import { Button, Input } from "@/components/ui";
  import {
    HardDrive, Upload, Download, FolderTree, Clock,
    FolderPlus, Trash2, FileText, ChevronRight, RefreshCw,
    Loader2, AlertCircle, CheckCircle, X,
  } from "lucide-svelte";
  import type { AppConfig, S3Config } from "@/types";
  import {
    parseS3ListResult,
    parseS3VersionsResult,
    type S3FileItem,
    type S3VersionItem,
  } from "@/storage/contracts";

  let s3Config = $state<S3Config | null>(null);
  let currentFolder = $state("");
  let folderPath = $state<string[]>([]);
  let files = $state<S3FileItem[]>([]);
  let loading = $state(false);
  let configLoading = $state(true);
  let configError = $state("");
  let fileState = $state<"loading" | "empty" | "ready" | "error">("loading");
  let error = $state("");
  let listRequestId = 0;
  let configRequestId = 0;
  let uploading = $state(false);
  let downloading = $state(false);
  let uploadProgress = $state("");
  let versionsOpen = $state(false);
  let versionsKey = $state("");
  let versions = $state<S3VersionItem[]>([]);
  let versionsLoading = $state(false);
  let versionsError = $state("");
  let versionRequestId = 0;
  let versionsDialog: HTMLDivElement | undefined = $state();
  let versionsTrigger: HTMLElement | null = null;
  let showNewFolder = $state(false);
  let newFolderName = $state("");

  async function loadConfig() {
    const requestId = ++configRequestId;
    configLoading = true;
    configError = "";
    try {
      const config = await invoke<AppConfig>("get_config");
      if (requestId !== configRequestId) return;
      s3Config = config.s3;
      currentFolder = "";
      folderPath = [];
      if (s3Config) {
        await loadFiles("", s3Config);
      } else {
        files = [];
        fileState = "empty";
      }
    } catch (e) {
      if (requestId === configRequestId) {
        configError = String(e);
      }
    } finally {
      if (requestId === configRequestId) configLoading = false;
    }
  }

  async function loadFiles(folder = currentFolder, config = s3Config) {
    if (!config) return;
    const requestId = ++listRequestId;
    loading = true;
    fileState = "loading";
    error = "";
    try {
      const result = parseS3ListResult(
        await invoke<unknown>("s3_list_files", { s3Config: config, folder }),
      );
      if (
        requestId !== listRequestId ||
        folder !== currentFolder ||
        config !== s3Config
      ) {
        return;
      }
      files = result.items;
      fileState = files.length === 0 ? "empty" : "ready";
    } catch (e) {
      if (
        requestId !== listRequestId ||
        folder !== currentFolder ||
        config !== s3Config
      ) {
        return;
      }
      error = String(e);
      files = [];
      fileState = "error";
    } finally {
      if (requestId === listRequestId) loading = false;
    }
  }

  async function refresh() {
    await loadFiles();
  }

  async function navigateTo(name: string) {
    currentFolder = currentFolder ? `${currentFolder}/${name}` : name;
    folderPath = [...folderPath, name];
    await loadFiles(currentFolder);
  }

  async function navigateToIndex(index: number) {
    folderPath = folderPath.slice(0, index + 1);
    currentFolder = folderPath.join("/");
    await loadFiles(currentFolder);
  }

  async function navigateToRoot() {
    folderPath = [];
    currentFolder = "";
    await loadFiles("");
  }

  async function uploadFiles() {
    if (!s3Config) return;
    const selected = await open({
      multiple: true,
      filters: [
        { name: "PDF", extensions: ["pdf"] },
        { name: "All", extensions: ["*"] },
      ],
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected.map(String) : [String(selected)];
    uploading = true;
    uploadProgress = "";
    error = "";
    try {
      for (let i = 0; i < paths.length; i++) {
        const name = paths[i].split(/[\\/]/).pop() || `file_${i}`;
        uploadProgress = `${i + 1}/${paths.length} - ${name}`;
        await invoke("s3_upload_file", {
          s3Config,
          localPath: paths[i],
          folder: currentFolder,
        });
      }
      uploadProgress = "";
      await loadFiles();
    } catch (e) {
      error = String(e);
    } finally {
      uploading = false;
    }
  }

  async function downloadFile(key: string, name: string) {
    if (!s3Config) return;
    const out = await save({
      defaultPath: name,
      filters: [{ name: "All", extensions: ["*"] }],
    });
    if (!out) return;
    downloading = true;
    error = "";
    try {
      await invoke("s3_download_file", {
        s3Config,
        remoteKey: key,
        localPath: out,
      });
    } catch (e) {
      error = `${t("storage.downloadError")}: ${key} -> ${out} (${String(e)})`;
    } finally {
      downloading = false;
    }
  }

  async function deleteFile(key: string) {
    if (!s3Config) return;
    const confirmed = await ask(`${t("storage.deleteConfirm")}\n\n${key}`, {
      title: t("storage.delete"),
      kind: "warning",
    });
    if (!confirmed) return;
    error = "";
    try {
      await invoke("s3_delete_file", { s3Config, remoteKey: key });
      await loadFiles();
    } catch (e) {
      error = String(e);
    }
  }

  async function showVersions(key: string, trigger?: HTMLElement) {
    if (!s3Config) return;
    const requestId = ++versionRequestId;
    if (trigger) {
      versionsTrigger = trigger;
    } else if (!versionsTrigger) {
      versionsTrigger =
        document.activeElement instanceof HTMLElement ? document.activeElement : null;
    }
    versionsKey = key;
    versionsOpen = true;
    versionsLoading = true;
    versions = [];
    versionsError = "";
    error = "";
    await tick();
    versionsDialog?.focus();
    try {
      const result = parseS3VersionsResult(
        await invoke<unknown>("s3_list_versions", { s3Config, remoteKey: key }),
      );
      if (requestId !== versionRequestId || versionsKey !== key) return;
      versions = result.versions;
    } catch (e) {
      if (requestId === versionRequestId && versionsKey === key) {
        versionsError = String(e);
      }
    } finally {
      if (requestId === versionRequestId) versionsLoading = false;
    }
  }

  function closeVersions() {
    versionRequestId++;
    versionsOpen = false;
    versionsLoading = false;
    const trigger = versionsTrigger;
    versionsTrigger = null;
    queueMicrotask(() => trigger?.focus());
  }

  async function deleteVersion(versionId: string) {
    if (!s3Config) return;
    const confirmed = await ask(
      `${t("storage.deleteVersionConfirm")}\n\n${versionsKey}\n${versionId}`,
      { title: t("storage.deleteVersion"), kind: "warning" },
    );
    if (!confirmed) return;
    error = "";
    try {
      await invoke("s3_delete_version", { s3Config, remoteKey: versionsKey, versionId });
      await showVersions(versionsKey);
    } catch (e) {
      versionsError = String(e);
    }
  }

  $effect(() => {
    if (!versionsOpen) return;
    const handleKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        closeVersions();
        return;
      }
      if (event.key !== "Tab" || !versionsDialog) return;
      const focusable = Array.from(
        versionsDialog.querySelectorAll<HTMLElement>(
          'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])',
        ),
      );
      if (focusable.length === 0) {
        event.preventDefault();
        versionsDialog.focus();
      } else if (
        event.shiftKey &&
        (document.activeElement === focusable[0] ||
          !versionsDialog.contains(document.activeElement))
      ) {
        event.preventDefault();
        focusable[focusable.length - 1].focus();
      } else if (
        !event.shiftKey &&
        (document.activeElement === focusable[focusable.length - 1] ||
          !versionsDialog.contains(document.activeElement))
      ) {
        event.preventDefault();
        focusable[0].focus();
      }
    };
    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
  });

  async function createFolder() {
    if (!s3Config || !newFolderName.trim()) return;
    error = "";
    try {
      const path = currentFolder ? `${currentFolder}/${newFolderName.trim()}` : newFolderName.trim();
      await invoke("s3_create_folder", { s3Config, folderName: path });
      newFolderName = "";
      showNewFolder = false;
      await loadFiles();
    } catch (e) {
      error = String(e);
    }
  }

  function formatSize(bytes: number): string {
    if (bytes === 0) return "-";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function formatDate(s: string): string {
    if (!s) return "-";
    try {
      return new Date(s).toLocaleString();
    } catch {
      return s;
    }
  }

  $effect(() => {
    loadConfig();
  });
</script>

<div class="flex flex-col h-full overflow-hidden">
  <!-- Header -->
  <div class="flex items-center h-12 px-6 border-b border-border bg-card shrink-0">
    <h1 class="text-base font-semibold text-foreground">{t("nav.storage")}</h1>
    <div class="flex-1"></div>
    {#if configLoading}
      <Loader2 size={15} class="animate-spin text-muted-foreground" />
    {:else if s3Config}
      <Button variant="outline" size="sm" onclick={uploadFiles} disabled={uploading} class="gap-1.5 mr-2">
        {#if uploading}
          <Loader2 size={14} class="animate-spin" />
        {:else}
          <Upload size={14} />
        {/if}
        {t("storage.upload")}
      </Button>
      <Button
        variant="outline"
        size="sm"
        onclick={() => showNewFolder = !showNewFolder}
        class="gap-1.5 mr-2"
        aria-label={t("storage.newFolder")}
      >
        <FolderPlus size={14} />
        {t("storage.newFolder")}
      </Button>
      <Button
        variant="ghost"
        size="sm"
        onclick={refresh}
        class="gap-1"
        title={t("storage.refresh")}
        aria-label={t("storage.refresh")}
      >
        <RefreshCw size={14} class={loading ? "animate-spin" : ""} />
      </Button>
    {:else}
      <span class="text-sm text-muted-foreground">{t("storage.noConfig")}</span>
    {/if}
  </div>

  <div class="flex-1 overflow-auto p-6">
    {#if configLoading}
      <div class="flex items-center justify-center h-full">
        <Loader2 size={24} class="animate-spin text-muted-foreground" />
      </div>
    {:else if configError}
      <div class="flex items-center justify-center h-full text-sm text-destructive">
        {configError}
      </div>
    {:else if !s3Config}
      <div class="flex flex-col items-center justify-center h-full gap-6">
        <div class="p-6 rounded-2xl bg-muted/50">
          <HardDrive size={48} class="text-muted-foreground/40 mx-auto" />
        </div>
        <div class="text-center space-y-1">
          <p class="text-muted-foreground">{t("settings.s3")}</p>
          <p class="text-xs text-muted-foreground/70">
            {t("storage.noConfig")}
          </p>
        </div>
      </div>
    {:else}
      <!-- Breadcrumb -->
      <div class="flex items-center gap-1 mb-4 text-sm">
        <button
          class="text-muted-foreground hover:text-foreground transition-colors"
          onclick={navigateToRoot}
        >
          {t("settings.s3")}
        </button>
        {#each folderPath as seg, i}
          <ChevronRight size={14} class="text-muted-foreground/50" />
          <button
            class="text-muted-foreground hover:text-foreground transition-colors"
            onclick={() => navigateToIndex(i)}
          >
            {seg}
          </button>
        {/each}
      </div>

      <!-- New folder input -->
      {#if showNewFolder}
        <div class="flex items-center gap-2 mb-4">
          <label for="new-folder-name" class="text-sm font-medium">{t("storage.folderName")}</label>
          <Input
            id="new-folder-name"
            bind:value={newFolderName}
            placeholder={t("storage.folderName")}
            class="w-60"
          />
          <Button size="sm" onclick={createFolder}>{t("storage.createFolder")}</Button>
          <Button
            variant="ghost"
            size="sm"
            title={t("storage.close")}
            aria-label={t("storage.close")}
            onclick={() => { showNewFolder = false; newFolderName = ""; }}
          >
            <X size={14} />
          </Button>
        </div>
      {/if}

      <!-- Upload progress -->
      {#if uploading}
        <div class="mb-4 p-2 rounded-lg bg-muted text-sm text-muted-foreground flex items-center gap-2">
          <Loader2 size={14} class="animate-spin" />
          {uploadProgress}
        </div>
      {/if}

      <!-- Download progress -->
      {#if downloading}
        <div class="mb-4 p-2 rounded-lg bg-muted text-sm text-muted-foreground flex items-center gap-2">
          <Loader2 size={14} class="animate-spin" />
          {t("storage.downloading")}
        </div>
      {/if}

      <!-- Error -->
      {#if error}
        <div class="mb-4 flex items-center gap-2 p-3 rounded-lg border border-destructive/30 bg-destructive/5">
          <AlertCircle size={16} class="text-destructive shrink-0" />
          <span class="text-sm text-destructive">{error}</span>
        </div>
      {/if}

      <!-- File list -->
      {#if loading}
        <div class="flex items-center justify-center py-12">
          <Loader2 size={24} class="animate-spin text-muted-foreground" />
        </div>
      {:else if fileState === "empty"}
        <div class="flex flex-col items-center justify-center py-12 text-muted-foreground">
          <FolderTree size={40} class="mb-3 opacity-40" />
          <p class="text-sm">{t("storage.empty")}</p>
        </div>
      {:else if fileState === "error"}
        <div class="py-12 text-center text-sm text-muted-foreground">
          {t("storage.loadFailed")}
        </div>
      {:else}
        <div class="space-y-1">
          {#each files as file (file.key)}
            <div
              class="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-accent transition-colors group"
            >
              {#if file.is_dir}
                <button
                  class="flex items-center gap-3 flex-1 text-left"
                  onclick={() => navigateTo(file.name)}
                >
                  <FolderTree size={18} class="text-muted-foreground shrink-0" />
                  <span class="text-sm text-foreground flex-1 truncate">{file.name}</span>
                </button>
              {:else}
                <FileText size={18} class="text-muted-foreground shrink-0" />
                <span class="text-sm text-foreground flex-1 truncate" title={file.key}>{file.name}</span>
                <span class="text-xs text-muted-foreground w-20 text-right">{formatSize(file.size)}</span>
                <span class="text-xs text-muted-foreground w-40 text-right">{formatDate(file.last_modified)}</span>
                <div class="flex items-center gap-1 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity">
                  <button
                    class="p-1 rounded hover:bg-muted text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    title={t("storage.download")}
                    aria-label={t("storage.download")}
                    onclick={() => downloadFile(file.key, file.name)}
                  >
                    <Download size={14} />
                  </button>
                  <button
                    class="p-1 rounded hover:bg-muted text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    title={t("storage.versions")}
                    aria-label={t("storage.versions")}
                    onclick={(event) => showVersions(file.key, event.currentTarget)}
                  >
                    <Clock size={14} />
                  </button>
                  <button
                    class="p-1 rounded hover:bg-muted text-destructive/70 hover:text-destructive focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    title={t("storage.delete")}
                    aria-label={t("storage.delete")}
                    onclick={() => deleteFile(file.key)}
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {/if}

      <!-- Versions panel -->
      {#if versionsOpen}
        <div
          class="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
          role="presentation"
        >
          <div
            bind:this={versionsDialog}
            role="dialog"
            aria-modal="true"
            aria-labelledby="storage-versions-title"
            tabindex="-1"
            class="bg-card border border-border rounded-xl shadow-lg w-[calc(100vw-2rem)] max-w-[500px] max-h-[70vh] flex flex-col"
          >
            <div class="flex items-center justify-between px-5 py-4 border-b border-border">
              <h3 id="storage-versions-title" class="text-sm font-medium text-foreground flex items-center gap-2 min-w-0">
                <Clock size={16} />
                <span class="truncate" title={versionsKey}>{t("storage.versions")}: {versionsKey}</span>
              </h3>
              <button
                class="text-muted-foreground hover:text-foreground"
                title={t("storage.close")}
                aria-label={t("storage.close")}
                onclick={closeVersions}
              >
                <X size={16} />
              </button>
            </div>
            <div class="flex-1 overflow-auto p-3">
              {#if versionsError}
                <p class="text-sm text-destructive text-center py-8">{versionsError}</p>
              {:else if versionsLoading}
                <div class="flex items-center justify-center py-8">
                  <Loader2 size={20} class="animate-spin text-muted-foreground" />
                </div>
              {:else if versions.length === 0}
                <p class="text-sm text-muted-foreground text-center py-8">{t("storage.noVersions")}</p>
              {:else}
                <div class="space-y-1">
                  {#each versions as ver}
                    <div class="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-accent text-sm">
                      {#if ver.is_latest}
                        <CheckCircle size={14} class="text-green-600 shrink-0" />
                      {:else}
                        <div class="w-3.5 shrink-0"></div>
                      {/if}
                      <span class="text-muted-foreground w-20 text-xs font-mono truncate" title={ver.version_id}>{ver.version_id.slice(0, 12)}...</span>
                      <span class="flex-1 text-muted-foreground">{formatSize(ver.size)}</span>
                      <span class="text-muted-foreground text-xs">{formatDate(ver.last_modified)}</span>
                      {#if !ver.is_latest}
                        <button
                          class="p-1 rounded hover:bg-muted text-destructive/70 hover:text-destructive"
                          title={t("storage.deleteVersion")}
                          aria-label={t("storage.deleteVersion")}
                          onclick={() => deleteVersion(ver.version_id)}
                        >
                          <Trash2 size={13} />
                        </button>
                      {/if}
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
        </div>
      {/if}
    {/if}
  </div>
</div>
