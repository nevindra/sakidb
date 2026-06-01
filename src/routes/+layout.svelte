<script>
  import '@fontsource-variable/plus-jakarta-sans';
  import '../app.css';
  import OracleDriverDialog from '$lib/components/sidebar/OracleDriverDialog.svelte';
  import { getAppState } from '$lib/stores';
  let { children } = $props();
  const app = getAppState();
</script>

{@render children()}

<!-- [Fix: Minor 5] Render OracleDriverDialog only if Oracle engine is supported -->
{#if app.availableEngines.includes('oracle')}
  <OracleDriverDialog />
{/if}
