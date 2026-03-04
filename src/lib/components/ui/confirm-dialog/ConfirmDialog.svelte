<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog';
  import { Button } from '$lib/components/ui/button';

  let {
    open = $bindable(false),
    title = 'Are you sure?',
    description = '',
    confirmLabel = 'Confirm',
    cancelLabel = 'Cancel',
    variant = 'destructive' as 'destructive' | 'default',
    loading = false,
    onconfirm,
  }: {
    open?: boolean;
    title?: string;
    description?: string;
    confirmLabel?: string;
    cancelLabel?: string;
    variant?: 'destructive' | 'default';
    loading?: boolean;
    onconfirm?: () => void | Promise<void>;
  } = $props();

  async function handleConfirm() {
    await onconfirm?.();
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
