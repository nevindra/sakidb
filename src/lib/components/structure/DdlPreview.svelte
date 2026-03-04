<script lang="ts">
  let { sql }: { sql: string } = $props();
</script>

{#if sql}
  <div class="mt-3">
    <span class="text-xs text-muted-foreground font-medium">DDL Preview</span>
    <pre class="mt-1 p-2 bg-card border border-border rounded text-xs font-mono text-foreground whitespace-pre-wrap max-h-32 overflow-auto">{sql}</pre>
  </div>
{/if}
