<script lang="ts">
  interface Props {
    activeZone: 'top' | 'right' | 'bottom' | 'left' | 'center' | null;
  }

  let { activeZone }: Props = $props();
</script>

{#if activeZone}
  <div class="absolute inset-0 z-50 pointer-events-none">
    {#if activeZone === 'top'}
      <div class="absolute inset-x-0 top-0 h-1/2 bg-primary/15 border-2 border-primary/30 rounded-md m-1"></div>
    {:else if activeZone === 'bottom'}
      <div class="absolute inset-x-0 bottom-0 h-1/2 bg-primary/15 border-2 border-primary/30 rounded-md m-1"></div>
    {:else if activeZone === 'left'}
      <div class="absolute inset-y-0 left-0 w-1/2 bg-primary/15 border-2 border-primary/30 rounded-md m-1"></div>
    {:else if activeZone === 'right'}
      <div class="absolute inset-y-0 right-0 w-1/2 bg-primary/15 border-2 border-primary/30 rounded-md m-1"></div>
    {:else if activeZone === 'center'}
      <div class="absolute inset-0 bg-primary/10 border-2 border-primary/20 rounded-md m-1"></div>
    {/if}
  </div>
{/if}
