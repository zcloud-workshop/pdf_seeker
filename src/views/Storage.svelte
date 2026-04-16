<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import { invoke } from "@tauri-apps/api/core";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { Button, Input } from "@/components/ui";
  import {
    HardDrive, Upload, Download, FolderTree, Clock,
    FolderPlus, Trash2, FileText, ChevronRight, RefreshCw,
    Loader2, AlertCircle, CheckCircle, X,
  } from "lucide-svelte";
  // @ts-ignore - module resolution for types
  import type { S3Config } from "@/lib/types";

  let s3Config = $state<S3Config | null>(null);
  let currentFolder = $state("");
  let folderPath = $state<string[]>([]);
  let files = $state<Array<{ key: string; name: string; size: number; lastModified: string; isDir: boolean }>>([]);
  let loading = $state(false);
  let error = $state("");
  let uploading = $state(false);
  let downloading = $state(false);
  let uploadProgress = $state("");
  let versionsOpen = $state(false);
  let versionsKey = $state("");
  let versions = $state<Array<{ versionId: string; size: number; lastModified: string; isLatest: boolean }>>([]);
  let versionsLoading = $state(false);
  let showNewFolder = $state(false);
  let newFolderName = $state("");

  async function loadConfig() {
    try {
      const config = await invoke<{ general: any; s3: S3Config | null }>("get_config");
      s3Config = config.s3;
    } catch (_) {}
  }

  async function loadFiles() {
    if (!s3Config) return;
    loading = true;
    error = "";
    try {
      const result = await invoke<{
        items: Array<{ key: string; name: string; size: number; lastModified: string; isDir: boolean }>;
        commonPrefixes: string[];
      }>("s3_list_files", { s3Config, folder: currentFolder });
      files = result.items;
    } catch (e) {
      error = String(e);
      files = [];
    } finally {
      loading = false;
    }
  }

  async function refresh() {
    await loadFiles();
  }

  async function navigateTo(name: string) {
    currentFolder = currentFolder ? `${currentFolder}/${name}` : name;
    folderPath = [...folderPath, name];
    await loadFiles();
  }

  async function navigateToIndex(index: number) {
    folderPath = folderPath.slice(0, index);
    currentFolder = folderPath.join("/");
    await loadFiles();
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
          remoteKey: currentFolder ? `${currentFolder}/${name}` : name,
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
        localPath: out as string,
      });
    } catch (e) {
      error = String(e);
    } finally {
      downloading = false;
    }
  }

  async function deleteFile(key: string) {
    if (!s3Config) return;
    error = "";
    try {
      await invoke("s3_delete_file", { s3Config, remoteKey: key });
      await loadFiles();
    } catch (e) {
      error = String(e);
    }
  }

  async function showVersions(key: string) {
    if (!s3Config) return;
    versionsKey = key;
    versionsOpen = true;
    versionsLoading = true;
    versions = [];
    error = "";
    try {
      const result = await invoke<{
        versions: Array<{ versionId: string; size: number; lastModified: string; isLatest: boolean }>;
        deleteMarkers: string[];
      }>("s3_list_versions", { s3Config, remoteKey: key });
      versions = result.versions;
    } catch (e) {
      error = String(e);
    } finally {
      versionsLoading = false;
    }
  }

  async function deleteVersion(versionId: string) {
    if (!s3Config) return;
    error = "";
    try {
      await invoke("s3_delete_version", { s3Config, remoteKey: versionsKey, versionId });
      await showVersions(versionsKey);
    } catch (e) {
      error = String(e);
    }
  }

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
    {#if s3Config}
      <Button variant="outline" size="sm" onclick={uploadFiles} disabled={uploading} class="gap-1.5 mr-2">
        {#if uploading}
          <Loader2 size={14} class="animate-spin" />
        {:else}
          <Upload size={14} />
        {/if}
        {t("storage.upload") || "Upload"}
      </Button>
      <Button variant="outline" size="sm" onclick={() => showNewFolder = !showNewFolder} class="gap-1.5 mr-2">
        <FolderPlus size={14} />
        {t("storage.newFolder") || "New Folder"}
      </Button>
      <Button variant="ghost" size="sm" onclick={refresh} class="gap-1">
        <RefreshCw size={14} class={loading ? "animate-spin" : ""} />
      </Button>
    {:else}
      <span class="text-sm text-muted-foreground">Please configure S3 in Settings</span>
    {/if}
  </div>

  <div class="flex-1 overflow-auto p-6">
    {#if !s3Config}
      <div class="flex flex-col items-center justify-center h-full gap-6">
        <div class="p-6 rounded-2xl bg-muted/50">
          <HardDrive size={48} class="text-muted-foreground/40 mx-auto" />
        </div>
        <div class="text-center space-y-1">
          <p class="text-muted-foreground">{t("settings.s3")}</p>
          <p class="text-xs text-muted-foreground/70">
            Configure S3 storage in Settings to enable cloud features.
          </p>
        </div>
      </div>
    {:else}
      <!-- Breadcrumb -->
      <div class="flex items-center gap-1 mb-4 text-sm">
        <button
          class="text-muted-foreground hover:text-foreground transition-colors"
          onclick={() => { folderPath = []; currentFolder = ""; loadFiles(); }}
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
          <Input
            value={newFolderName}
            onchange={(e) => (newFolderName = (e.target as HTMLInputElement).value)}
            placeholder="Folder name"
            class="w-60"
          />
          <Button size="sm" onclick={createFolder}>Create</Button>
          <Button variant="ghost" size="sm" onclick={() => { showNewFolder = false; newFolderName = ""; }}>
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
          Downloading...
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
      {:else if files.length === 0}
        <div class="flex flex-col items-center justify-center py-12 text-muted-foreground">
          <FolderTree size={40} class="mb-3 opacity-40" />
          <p class="text-sm">Empty folder</p>
        </div>
      {:else}
        <div class="space-y-1">
          {#each files as file (file.key)}
            <div
              class="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-accent transition-colors group"
            >
              {#if file.isDir}
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
                <span class="text-xs text-muted-foreground w-40 text-right">{formatDate(file.lastModified)}</span>
                <div class="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                  <button
                    class="p-1 rounded hover:bg-muted text-muted-foreground hover:text-foreground"
                    title="Download"
                    onclick={() => downloadFile(file.key, file.name)}
                  >
                    <Download size={14} />
                  </button>
                  <button
                    class="p-1 rounded hover:bg-muted text-muted-foreground hover:text-foreground"
                    title="Versions"
                    onclick={() => showVersions(file.key)}
                  >
                    <Clock size={14} />
                  </button>
                  <button
                    class="p-1 rounded hover:bg-muted text-destructive/70 hover:text-destructive"
                    title="Delete"
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
        <!-- svelte-ignore a11y_no_static_element_interactions a11y_interactive_supports_focus -->
        <div
          class="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
          onclick={() => { versionsOpen = false; }}
          role="dialog"
        >
          <div
            class="bg-card border border-border rounded-xl shadow-lg w-[500px] max-h-[70vh] flex flex-col"
            onclick={(e) => e.stopPropagation()}
          >
            <div class="flex items-center justify-between px-5 py-4 border-b border-border">
              <h3 class="text-sm font-medium text-foreground flex items-center gap-2">
                <Clock size={16} />
                Versions — {versionsKey.split("/").pop()}
              </h3>
              <button class="text-muted-foreground hover:text-foreground" onclick={() => { versionsOpen = false; }}>
                <X size={16} />
              </button>
            </div>
            <div class="flex-1 overflow-auto p-3">
              {#if versionsLoading}
                <div class="flex items-center justify-center py-8">
                  <Loader2 size={20} class="animate-spin text-muted-foreground" />
                </div>
              {:else if versions.length === 0}
                <p class="text-sm text-muted-foreground text-center py-8">No versions found</p>
              {:else}
                <div class="space-y-1">
                  {#each versions as ver}
                    <div class="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-accent text-sm">
                      {#if ver.isLatest}
                        <CheckCircle size={14} class="text-green-600 shrink-0" />
                      {:else}
                        <div class="w-3.5 shrink-0"></div>
                      {/if}
                      <span class="text-muted-foreground w-20 text-xs font-mono truncate">{ver.versionId.slice(0, 12)}...</span>
                      <span class="flex-1 text-muted-foreground">{formatSize(ver.size)}</span>
                      <span class="text-muted-foreground text-xs">{formatDate(ver.lastModified)}</span>
                      {#if !ver.isLatest}
                        <button
                          class="p-1 rounded hover:bg-muted text-destructive/70 hover:text-destructive"
                          title="Delete version"
                          onclick={() => deleteVersion(ver.versionId)}
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
