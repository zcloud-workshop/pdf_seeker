<script lang="ts">
  import { X as Icon_X } from "lucide-svelte";

  let { open = $bindable(false) }: { open: boolean } = $props();

  let dialogEl = $state<HTMLDialogElement | undefined>(undefined);

  $effect(() => {
    if (open && dialogEl) {
      dialogEl.showModal();
    } else if (!open && dialogEl?.open) {
      dialogEl.close();
    }
  });

  function onClose() {
    open = false;
  }

  const shortcuts = [
    { keys: "Ctrl + O", desc: "Open file" },
    { keys: "Ctrl + S", desc: "Save file" },
    { keys: "Ctrl + Z", desc: "Undo" },
    { keys: "Ctrl + Shift + Z", desc: "Redo" },
    { keys: "Ctrl + Y", desc: "Redo (alternative)" },
    { keys: "Ctrl + =", desc: "Zoom in" },
    { keys: "Ctrl + -", desc: "Zoom out" },
    { keys: "Ctrl + 0", desc: "Reset zoom" },
    { keys: "Arrow Left / Right", desc: "Previous / Next page" },
    { keys: "F11", desc: "Toggle fullscreen" },
  ];
</script>

<dialog
  bind:this={dialogEl}
  onclose={onClose}
  class="backdrop:bg-black/40 bg-transparent p-0 m-auto"
>
  <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
  <div class="bg-card border border-border rounded-xl shadow-xl w-[420px] max-h-[80vh] overflow-auto p-0" onclick={(e) => { if ((e.target as HTMLElement).tagName === "DIALOG") onClose(); }}>
    <div class="flex items-center justify-between px-5 py-4 border-b border-border">
      <h2 class="text-sm font-semibold text-foreground">Keyboard Shortcuts</h2>
      <button onclick={onClose} class="text-muted-foreground hover:text-foreground transition-colors">
        <Icon_X size={16} />
      </button>
    </div>
    <div class="px-5 py-3 space-y-2">
      {#each shortcuts as s}
        <div class="flex items-center justify-between py-1.5">
          <span class="text-sm text-muted-foreground">{s.desc}</span>
          <kbd class="text-xs font-mono px-2 py-1 rounded bg-muted border border-border text-foreground">{s.keys}</kbd>
        </div>
      {/each}
    </div>
  </div>
</dialog>
