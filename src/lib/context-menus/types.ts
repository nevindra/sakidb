import type { EngineCapabilities } from '$lib/types';

export interface MenuContext {
  capabilities?: EngineCapabilities | null;
  isConnected?: boolean;
  isConnecting?: boolean;
  isDbConnected?: boolean;
  hasMultiDatabase?: boolean;
  isHistory?: boolean;
  engineType?: string;
}

export interface MenuItemDef {
  id: string;
  label: string;
  variant?: 'destructive';
  when?: (ctx: MenuContext) => boolean;
}

export interface MenuSeparatorDef {
  kind: 'separator';
  when?: (ctx: MenuContext) => boolean;
}

export type MenuEntry = MenuItemDef | MenuSeparatorDef;
