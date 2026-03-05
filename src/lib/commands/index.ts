export type { Command, CommandContext, CommandDefinition } from './types';
export { commandDefinitions } from './definitions';
export {
  initRegistry,
  setContexts,
  getActiveContexts,
  registerAction,
  registerActions,
  getCommands,
  getActiveCommands,
  getRecentCommands,
  getKeybinding,
  setKeybinding,
  resetKeybinding,
  resetAllKeybindings,
  executeCommand,
  fuzzyMatch,
  findConflict,
  formatKeybinding,
  normalizeKeybinding,
  markComingSoon,
  attachGlobalKeyListener,
  detachGlobalKeyListener,
} from './registry.svelte';
