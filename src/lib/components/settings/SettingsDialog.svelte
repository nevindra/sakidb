<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog';
  import { setRecordingMode } from '$lib/commands';
  import KeybindingsSettings from './KeybindingsSettings.svelte';

  let { open = $bindable(false) }: { open: boolean } = $props();

  function handleOpenChange(isOpen: boolean) {
    open = isOpen;
    if (!isOpen) setRecordingMode(false);
  }
</script>

<Dialog.Root open={open} onOpenChange={handleOpenChange}>
  <Dialog.Content class="sm:max-w-[720px] max-h-[80vh] flex flex-col">
    <Dialog.Header>
      <Dialog.Title>Settings</Dialog.Title>
      <Dialog.Description>Configure SakiDB preferences and keyboard shortcuts.</Dialog.Description>
    </Dialog.Header>
    <div class="flex-1 overflow-y-auto -mx-6 px-6">
      <KeybindingsSettings />
    </div>
  </Dialog.Content>
</Dialog.Root>
