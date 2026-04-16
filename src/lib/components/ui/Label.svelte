<script lang="ts">
  import { cn } from "@/utils/cn";

  interface Props {
    class?: string;
    children?: import("svelte").Snippet;
  }

  let { class: className = "", children }: Props = $props();
</script>

<label
  class={cn(
    "text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70",
    className,
  )}
>
  {#if children}
    {@render children()}
  {/if}
</label>
