<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import * as Dialog from '$lib/components/ui/dialog';
  import { Button } from '$lib/components/ui/button';
  import { Checkbox } from '$lib/components/ui/checkbox';

  let {
    open = $bindable(false),
    onhide,
  }: {
    open: boolean;
    onhide?: () => void;
  } = $props();

  let dontShowAgain = $state(false);

  async function handleClose() {
    if (dontShowAgain) {
      try {
        await invoke('set_preference', { key: 'hide_edit_tutorial', value: 'true' });
      } catch {
        // silently ignore — preference just won't persist
      }
      onhide?.();
    }
    open = false;
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content showCloseButton={true} class="max-w-sm">
    <Dialog.Header>
      <Dialog.Title class="text-sm font-semibold">Edit Mode</Dialog.Title>
      <Dialog.Description class="text-xs text-muted-foreground">
        You're now in edit mode. Here's how it works:
      </Dialog.Description>
    </Dialog.Header>

    <div class="space-y-3 text-xs text-foreground/90 py-1">
      <div class="px-2.5 py-2 rounded-md bg-accent/20 text-foreground/90 text-[11px]">
        <span class="font-medium">Right-click any row</span> to open the context menu with all edit actions (Edit Row, Insert Row, Delete Row).
      </div>

      <div class="flex gap-2.5">
        <span class="shrink-0 w-5 h-5 rounded bg-success/10 text-success flex items-center justify-center text-[11px] font-bold">+</span>
        <div>
          <span class="font-medium">Add Row</span> — click the toolbar button or right-click → Insert Row to open a form dialog.
        </div>
      </div>

      <div class="flex gap-2.5">
        <span class="shrink-0 w-5 h-5 rounded bg-primary/10 text-primary flex items-center justify-center text-[11px] font-bold">✎</span>
        <div>
          <span class="font-medium">Edit</span> — double-click a cell for quick inline edits, or right-click → Edit Row for a full form dialog.
        </div>
      </div>

      <div class="flex gap-2.5">
        <span class="shrink-0 w-5 h-5 rounded bg-destructive/10 text-destructive flex items-center justify-center text-[11px] font-bold">×</span>
        <div>
          <span class="font-medium">Delete</span> — click row numbers to select, then press <kbd class="px-1 py-0.5 rounded bg-muted text-[10px] font-mono">Delete</kbd>, or right-click → Delete Row.
        </div>
      </div>

      <div class="flex gap-2.5">
        <span class="shrink-0 w-5 h-5 rounded bg-accent/30 text-foreground flex items-center justify-center text-[11px] font-bold">↩</span>
        <div>
          <span class="font-medium">Undo</span> with <kbd class="px-1 py-0.5 rounded bg-muted text-[10px] font-mono">Ctrl+Z</kbd>. All changes are batched — click <span class="font-medium">Apply</span> to commit or <span class="font-medium">Discard</span> to cancel.
        </div>
      </div>
    </div>

    <div class="flex items-center gap-2 pt-2 border-t border-border/40">
      <Checkbox id="dont-show-tutorial" bind:checked={dontShowAgain} />
      <label for="dont-show-tutorial" class="text-[11px] text-muted-foreground cursor-pointer select-none">
        Don't show this again
      </label>
    </div>

    <Dialog.Footer>
      <Button size="sm" onclick={handleClose}>Got it</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
