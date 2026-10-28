export interface PaletteCommand {
  label: string;
  shortcut?: string;
  disabled?: boolean;
  run: () => unknown;
}

/** Enabled commands whose label contains every word of `query`, in their original order. */
export function filterCommands(commands: PaletteCommand[], query: string): PaletteCommand[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  return commands.filter(
    (c) => !c.disabled && words.every((word) => c.label.toLowerCase().includes(word)),
  );
}
