import { isMac } from './keys';

const cmd = isMac ? '⌘' : 'Ctrl+';
const alt = isMac ? '⌥' : 'Alt+';

/**
 * Every key and gesture, grouped by the work it belongs to, for the shortcut
 * sheet. None is shown anywhere else, so this is the one place they can be
 * found without reading the code.
 */
export const SHORTCUTS: { group: string; items: [string, string][] }[] = [
  {
    group: 'Files',
    items: [
      [`${cmd}O`, 'Open folder'],
      [`${cmd}S / ${cmd}Shift+S`, 'Save project / save as a new file'],
      [`${cmd}Z / ${cmd}Y`, 'Undo / redo'],
      [`${cmd}Shift+P`, 'Search commands'],
      [`${cmd},`, 'Settings'],
    ],
  },
  {
    group: 'Looking around',
    items: [
      ['Scroll', 'Intensity up or down'],
      [`${cmd}Scroll`, 'Zoom about the pointer (add Shift for fine steps)'],
      ['Shift+Scroll', 'Move along the axis'],
      ['Z', 'Zoom: drag a region; Z again for horizontal, vertical or box'],
      ['Shift+Z', 'Zoom out'],
      ['P, or hold Space', 'Pan'],
      ['F / H', 'Full / Fit height'],
      ['+ / −', 'Intensity'],
      ['Shift+← / →', 'Previous / next zoom'],
      ['M', 'Set range'],
      ['C', 'Crosshair'],
      ['Esc', 'Select (leave the current tool)'],
    ],
  },
  {
    group: 'Processing',
    items: [
      ['Shift+P', 'Manual phase: drag for PH0, right-drag for PH1, Enter applies'],
      ['B', 'Auto baseline'],
      [
        'Shift+B',
        'Baseline through points: click adds, drag moves, double-click removes, Enter applies',
      ],
      ['L', 'Reference to solvent'],
    ],
  },
  {
    group: 'Analysis',
    items: [
      ['K', 'Pick in region (peaks above the threshold line)'],
      [`${cmd}K`, 'Pick one by one'],
      ['I', 'Integrate region'],
      ['J', 'Mark multiplet'],
      [
        'Click a hydrogen',
        'Pick its hydrogens, then click their signal on the spectrum to assign them; Esc lets go',
      ],
      [
        'Click a hydrogen, multiplet window open',
        'Assign that signal to those hydrogens, or take them away',
      ],
      [
        'Point at a signal or a hydrogen',
        'The other lights up, to check an assignment at a glance',
      ],
      ['Right-click a label', 'Rename or remove a peak'],
      ['Shift+drag', 'Move a label, integral curve, bracket or multiplet'],
      ['Drag a bracket end', "Change an integral's range"],
    ],
  },
  {
    group: 'Several spectra',
    items: [
      ['Click a trace', 'Make that spectrum the one being worked on'],
      ['Shift+↑ / ↓', 'Next or previous spectrum in a stack'],
      ['← / → on the plot', 'Step through peaks, integrals or multiplets (↑ / ↓ chooses which)'],
      ['Enter / Delete on the plot', 'Name the peak reached; remove what was reached'],
      [`${alt}Scroll`, 'Scale the row under the pointer'],
      ['Double-click a name', 'Rename it in the list'],
    ],
  },
];
