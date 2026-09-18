<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import { Button } from "@/components/ui";
  import { currentView } from "@/stores";
  import { ScanLine, Zap, FolderCog } from "lucide-svelte";

  let { onDismiss }: { onDismiss: () => void } = $props();

  function goToSettings() {
    currentView.set("settings");
    onDismiss();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
  onclick={onDismiss}
>
  <div
    class="bg-background border border-border rounded-xl shadow-2xl p-6 max-w-md w-full mx-4 space-y-4"
    onclick={(e) => e.stopPropagation()}
  >
    <div class="flex items-center gap-3">
      <div class="flex items-center justify-center w-10 h-10 rounded-full bg-primary/10">
        <ScanLine size={20} class="text-primary" />
      </div>
      <h2 class="text-lg font-semibold text-foreground">
        {t("settings.ocrSetupTitle")}
      </h2>
    </div>

    <p class="text-sm text-muted-foreground">
      {t("settings.ocrSetupMessage")}
    </p>

    <div class="flex flex-col gap-2 pt-1">
      <Button class="justify-start gap-2" onclick={() => goToSettings()}>
        <Zap size={16} />
        <div class="text-left">
          <div>{t("settings.ocrSetupPaddle")}</div>
          <div class="text-xs opacity-70 font-normal">PP-OCRv5, 100+ languages, Metal GPU</div>
        </div>
      </Button>
      <Button variant="outline" class="justify-start gap-2" onclick={() => goToSettings()}>
        <FolderCog size={16} />
        <div class="text-left">
          <div>{t("settings.ocrSetupCustom")}</div>
          <div class="text-xs opacity-70 font-normal">MNN format models</div>
        </div>
      </Button>
    </div>

    <button
      class="text-xs text-muted-foreground hover:text-foreground"
      onclick={onDismiss}
    >
      {t("settings.ocrSkip")}
    </button>
  </div>
</div>
