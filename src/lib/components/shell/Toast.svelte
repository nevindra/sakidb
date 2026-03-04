<script lang="ts">
  import { onMount } from 'svelte';
  import { X, AlertTriangle } from '@lucide/svelte';

  let { message, onDismiss }: { message: string; onDismiss: () => void } = $props();

  onMount(() => {
    const timer = setTimeout(onDismiss, 5000);
    return () => clearTimeout(timer);
  });
</script>

<div class="fixed top-4 right-4 z-50 max-w-md animate-slide-in">
  <div class="bg-card border border-destructive/30 rounded-lg px-4 py-3 shadow-lg flex items-start gap-3">
    <AlertTriangle class="h-4 w-4 text-destructive shrink-0 mt-0.5" />
    <p class="text-sm text-foreground flex-1 break-words">{message}</p>
    <button
      onclick={onDismiss}
      class="text-muted-foreground hover:text-foreground shrink-0"
    >
      <X class="h-4 w-4" />
    </button>
  </div>
</div>

<style>
  @keyframes slide-in {
    from { transform: translateX(100%); opacity: 0; }
    to { transform: translateX(0); opacity: 1; }
  }
  .animate-slide-in {
    animation: slide-in 0.2s ease-out;
  }
</style>
