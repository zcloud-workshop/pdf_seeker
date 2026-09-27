<script lang="ts">
  import { cn } from "@/utils/cn";

  interface Props {
    message: string;
    class?: string;
    children?: import("svelte").Snippet;
  }

  let { message, class: className = "", children }: Props = $props();
  let visible = $state(false);
  const tooltipId = `tooltip-${Math.random().toString(36).slice(2, 10)}`;
</script>

<div
  class="relative inline-flex"
  role="group"
  aria-describedby={visible ? tooltipId : undefined}
  onmouseenter={() => (visible = true)}
  onmouseleave={() => (visible = false)}
  onfocusin={() => (visible = true)}
  onfocusout={() => (visible = false)}
>
  {#if children}
    {@render children()}
  {/if}
  {#if visible}
    <div
      id={tooltipId}
      role="tooltip"
      class={cn(
        "absolute z-50 bottom-full left-1/2 -translate-x-1/2 mb-2 px-3 py-1.5 text-xs rounded-md bg-popover text-popover-foreground border shadow-md whitespace-nowrap pointer-events-none",
        className,
      )}
    >
      {message}
    </div>
  {/if}
</div>
