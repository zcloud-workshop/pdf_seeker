<script lang="ts">
  import { cn } from "@/utils/cn";

  interface Props {
    message: string;
    placement?: "top" | "bottom" | "auto";
    class?: string;
    children?: import("svelte").Snippet;
  }

  let { message, placement = "auto", class: className = "", children }: Props = $props();
  let visible = $state(false);
  let triggerEl: HTMLElement | null = $state(null);
  let coords = $state({ top: 0, left: 0 });
  let actualPlacement = $state<"top" | "bottom">("top");

  function portal(node: HTMLElement) {
    if (typeof document !== "undefined") {
      document.body.appendChild(node);
    }
    return {
      destroy() {
        if (node.parentNode) {
          node.parentNode.removeChild(node);
        }
      },
    };
  }

  function handleOpen() {
    if (!triggerEl) return;
    const rect = triggerEl.getBoundingClientRect();
    const isNearTop = rect.top < 70;
    const isNearBottom = typeof window !== "undefined" && rect.bottom > window.innerHeight - 70;

    let dir: "top" | "bottom" = "top";
    if (placement === "bottom" || isNearTop) {
      dir = "bottom";
    } else if (placement === "top" || isNearBottom) {
      dir = "top";
    }

    actualPlacement = dir;
    if (dir === "bottom") {
      coords = {
        top: rect.bottom + 6,
        left: rect.left + rect.width / 2,
      };
    } else {
      coords = {
        top: rect.top - 6,
        left: rect.left + rect.width / 2,
      };
    }
    visible = true;
  }

  function handleClose() {
    visible = false;
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  bind:this={triggerEl}
  class="relative inline-flex"
  role="group"
  onmouseenter={handleOpen}
  onmouseleave={handleClose}
  onfocus={handleOpen}
  onblur={handleClose}
>
  {#if children}
    {@render children()}
  {/if}
</div>

{#if visible}
  <div
    use:portal
    style="top: {coords.top}px; left: {coords.left}px; transform: translate(-50%, {actualPlacement === 'bottom' ? '0' : '-100%'});"
    class={cn(
      "fixed z-[10000] px-2.5 py-1 text-xs rounded-md bg-popover text-popover-foreground border border-border shadow-xl whitespace-nowrap pointer-events-none animate-in fade-in zoom-in-95 duration-150 select-none",
      className,
    )}
  >
    {message}
  </div>
{/if}
