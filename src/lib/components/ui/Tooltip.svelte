<script lang="ts">
  import { cn } from "@/utils/cn";

  interface Props {
    message: string;
    class?: string;
    children?: import("svelte").Snippet;
  }

  let { message, class: className = "", children }: Props = $props();
  let visible = $state(false);
</script>

<div class="relative inline-flex" role="group">
  <div
    onmouseenter={() => (visible = true)}
    onmouseleave={() => (visible = false)}
    onfocus={() => (visible = true)}
    onblur={() => (visible = false)}
  >
    {#if children}
      {@render children()}
    {/if}
  </div>
  {#if visible}
    <div
      class={cn(
        "absolute z-50 bottom-full left-1/2 -translate-x-1/2 mb-2 px-3 py-1.5 text-xs rounded-md bg-popover text-popover-foreground border shadow-md whitespace-nowrap pointer-events-none",
        className,
      )}
    >
      {message}
    </div>
  {/if}
</div>
