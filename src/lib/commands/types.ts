export type CommandContext = 'global' | 'query-tab' | 'data-tab' | 'structure-tab' | 'erd-tab' | 'sidebar' | 'connected';

export interface CommandDefinition {
  id: string;
  label: string;
  category: string;
  defaultKeybinding: string | null;
  contexts: CommandContext[];
}

export interface Command extends CommandDefinition {
  action: () => void | Promise<void>;
  enabled: boolean;
  comingSoon: boolean;
}
