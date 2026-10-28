import { expect, it } from 'vitest';
import { filterCommands, type PaletteCommand } from './palette';

const run = () => undefined;
const commands: PaletteCommand[] = [
  { label: 'Export view as SVG…', run },
  { label: 'Export peaks as CSV…', run, disabled: true },
  { label: 'Zoom out', shortcut: 'Shift+Z', run },
];

it('matches every word in any order and hides disabled commands', () => {
  expect(filterCommands(commands, 'svg export').map((c) => c.label)).toEqual([
    'Export view as SVG…',
  ]);
  expect(filterCommands(commands, 'csv')).toEqual([]);
  expect(filterCommands(commands, '  ')).toHaveLength(2);
});
