<script>
  import '@fontsource-variable/plus-jakarta-sans';
  import '../app.css';
  import OracleDriverDialog from '$lib/components/sidebar/OracleDriverDialog.svelte';
  let { children } = $props();
</script>

{@render children()}

<OracleDriverDialog />
