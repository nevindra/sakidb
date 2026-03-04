<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';

  let {
    open = $bindable(false),
    title = '',
    description = '',
    label = '',
    placeholder = '',
    initialValue = '',
    confirmLabel = 'Confirm',
    cancelLabel = 'Cancel',
    loading = false,
    onconfirm,
  }: {
    open?: boolean;
    title?: string;
    description?: string;
    label?: string;
    placeholder?: string;
    initialValue?: string;
    confirmLabel?: string;
    cancelLabel?: string;
    loading?: boolean;
    onconfirm?: (value: string) => void | Promise<void>;
  } = $props();

  let value = $state('');

  $effect(() => {
    if (open) {
      value = initialValue;
    }
  });

  async function handleConfirm() {
    if (!value.trim()) return;
    await onconfirm?.(value.trim());
    open = false;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleConfirm();
    }
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
    <div class="py-2">
      {#if label}
        <label for="input-dialog-field" class="text-sm font-medium mb-1.5 block">{label}</label>
      {/if}
      <Input
        id="input-dialog-field"
        bind:value
        {placeholder}
        onkeydown={handleKeydown}
      />
    </div>
    <Dialog.Footer>
      <Button variant="outline" size="sm" onclick={() => (open = false)} disabled={loading}>
        {cancelLabel}
      </Button>
      <Button
        size="sm"
        onclick={handleConfirm}
        disabled={loading || !value.trim()}
      >
        {confirmLabel}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
