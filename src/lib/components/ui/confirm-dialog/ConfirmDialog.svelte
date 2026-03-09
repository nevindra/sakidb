<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog';
  import { Button } from '$lib/components/ui/button';
  import { Checkbox } from '$lib/components/ui/checkbox';

  let {
    open = $bindable(false),
    title = 'Are you sure?',
    description = '',
    confirmLabel = 'Confirm',
    cancelLabel = 'Cancel',
    variant = 'destructive' as 'destructive' | 'default',
    loading = false,
    showCascade = false,
    onconfirm,
  }: {
    open?: boolean;
    title?: string;
    description?: string;
    confirmLabel?: string;
    cancelLabel?: string;
    variant?: 'destructive' | 'default';
    loading?: boolean;
    showCascade?: boolean;
    onconfirm?: (cascade?: boolean) => void | Promise<void>;
  } = $props();

  let cascade = $state(false);

  // Reset cascade when dialog closes
  $effect(() => {
    if (!open) cascade = false;
  });

  async function handleConfirm() {
    await onconfirm?.(showCascade ? cascade : undefined);
    open = false;
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content showCloseButton={false} class="max-w-sm">
    <Dialog.Header>
      <Dialog.Title>{title}</Dialog.Title>
      {#if description}
        <Dialog.Description>{description}</Dialog.Description>
      {/if}
    </Dialog.Header>
    {#if showCascade}
      <div class="flex items-center gap-2 py-1">
        <Checkbox id="cascade-check" bind:checked={cascade} />
        <label for="cascade-check" class="text-sm font-normal cursor-pointer select-none">CASCADE (also drop dependent objects)</label>
      </div>
    {/if}
    <Dialog.Footer>
      <Button variant="outline" size="sm" onclick={() => (open = false)} disabled={loading}>
        {cancelLabel}
      </Button>
      <Button
        variant={variant}
        size="sm"
        onclick={handleConfirm}
        disabled={loading}
      >
        {confirmLabel}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
