/** Named colour schemes for the whole app: interface colours and trace colours.
 *
 * The palettes are the published colour values of each scheme (Catppuccin,
 * Nord, Solarized and Gruvbox are all MIT licensed); only the values are used,
 * mapped onto the variables in `app.css`.
 */
export interface Palette {
  name: string;
  /** Whether the scheme is dark, so `color-scheme` and form controls match. */
  dark: boolean;
  colors: Record<string, string>;
  /** Trace colours, the first being the single-spectrum colour. */
  traces: string[];
}

/** Choices that use the stylesheet's own colours rather than a palette. */
export const BUILT_IN = ['system', 'light', 'dark'] as const;

/** A scheme of one's own, chosen colour by colour in the settings. */
export const CUSTOM = 'custom';

/** What a custom scheme sets, and what each one is called where it is chosen. */
export const CUSTOM_COLOURS: [string, string][] = [
  ['--bg', 'Background'],
  ['--panel', 'Panels'],
  ['--border', 'Lines'],
  ['--fg', 'Text'],
  ['--muted', 'Quiet text'],
  ['--accent', 'Actions'],
  ['--trace', 'Spectrum'],
  ['--integral', 'Integrals'],
];

/** Where a scheme of one's own starts: the plate under the lamp. */
export const DEFAULT_CUSTOM: Record<string, string> = {
  '--bg': '#16131d',
  '--panel': '#1d1926',
  '--border': '#312b3f',
  '--fg': '#ebe8f3',
  '--muted': '#a6a0b8',
  '--accent': '#ddd8ea',
  '--trace': '#82a8ff',
  '--integral': '#b9b3c8',
};

/** A scheme in the app's roles: what acts and what is on in the scheme's own
 *  accent, as one chooses a scheme for its colours; the spectra in its
 *  colours. */
const scheme = (
  name: string,
  dark: boolean,
  [bg, panel, border, fg, muted, accent, errorBg, errorFg]: string[],
  traces: string[],
): Palette => {
  const first = traces[0];
  return {
    name,
    dark,
    colors: {
      '--bg': bg,
      '--panel': panel,
      '--border': border,
      '--fg': fg,
      '--muted': muted,
      '--accent': accent,
      '--trace': first,
      '--integral': muted,
      '--error-bg': errorBg,
      '--error-fg': errorFg,
    },
    traces,
  };
};

export const PALETTES: Record<string, Palette> = {
  'catppuccin-latte': scheme(
    'Catppuccin Latte',
    false,
    ['#eff1f5', '#e6e9ef', '#ccd0da', '#4c4f69', '#6c6f85', '#1e66f5', '#f2d5d8', '#d20f39'],
    ['#1e66f5', '#fe640b', '#40a02b', '#8839ef', '#d20f39', '#179299'],
  ),
  'catppuccin-frappe': scheme(
    'Catppuccin Frappé',
    true,
    ['#303446', '#292c3c', '#414559', '#c6d0f5', '#a5adce', '#8caaee', '#402b31', '#e78284'],
    ['#8caaee', '#ef9f76', '#a6d189', '#ca9ee6', '#e78284', '#81c8be'],
  ),
  'catppuccin-macchiato': scheme(
    'Catppuccin Macchiato',
    true,
    ['#24273a', '#1e2030', '#363a4f', '#cad3f5', '#a5adcb', '#8aadf4', '#3d2830', '#ed8796'],
    ['#8aadf4', '#f5a97f', '#a6da95', '#c6a0f6', '#ed8796', '#8bd5ca'],
  ),
  'catppuccin-mocha': scheme(
    'Catppuccin Mocha',
    true,
    ['#1e1e2e', '#181825', '#313244', '#cdd6f4', '#a6adc8', '#89b4fa', '#3c2733', '#f38ba8'],
    ['#89b4fa', '#fab387', '#a6e3a1', '#cba6f7', '#f38ba8', '#94e2d5'],
  ),
  nord: scheme(
    'Nord',
    true,
    ['#2e3440', '#3b4252', '#4c566a', '#eceff4', '#a9b1c0', '#88c0d0', '#432f33', '#bf616a'],
    ['#88c0d0', '#d08770', '#a3be8c', '#b48ead', '#bf616a', '#8fbcbb'],
  ),
  'solarized-light': scheme(
    'Solarized Light',
    false,
    ['#fdf6e3', '#eee8d5', '#d9d2bd', '#586e75', '#93a1a1', '#268bd2', '#f6e0dc', '#dc322f'],
    ['#268bd2', '#cb4b16', '#859900', '#6c71c4', '#dc322f', '#2aa198'],
  ),
  'solarized-dark': scheme(
    'Solarized Dark',
    true,
    ['#002b36', '#073642', '#0f4a58', '#93a1a1', '#657b83', '#268bd2', '#3a2220', '#dc322f'],
    ['#268bd2', '#cb4b16', '#859900', '#6c71c4', '#dc322f', '#2aa198'],
  ),
  'gruvbox-light': scheme(
    'Gruvbox Light',
    false,
    ['#fbf1c7', '#ebdbb2', '#d5c4a1', '#3c3836', '#7c6f64', '#076678', '#f2e0c0', '#9d0006'],
    ['#076678', '#af3a03', '#79740e', '#8f3f71', '#9d0006', '#427b58'],
  ),
  'gruvbox-dark': scheme(
    'Gruvbox Dark',
    true,
    ['#282828', '#3c3836', '#504945', '#ebdbb2', '#a89984', '#83a598', '#432b2b', '#fb4934'],
    ['#83a598', '#fe8019', '#b8bb26', '#d3869b', '#fb4934', '#8ec07c'],
  ),
};

export type ThemeName = (typeof BUILT_IN)[number] | string;

export const isTheme = (value: unknown): value is ThemeName =>
  typeof value === 'string' &&
  (value === CUSTOM || BUILT_IN.some((b) => b === value) || value in PALETTES);

/** WCAG contrast ratio of two #rrggbb colours. */
export function contrast(a: string, b: string): number {
  const light = (hex: string) => {
    const [r, g, bl] = [1, 3, 5].map((i) => {
      const c = parseInt(hex.slice(i, i + 2), 16) / 255;
      return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    });
    return 0.2126 * r + 0.7152 * g + 0.0722 * bl;
  };
  const [hi, lo] = [light(a), light(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

/** What a scheme of one's own falls short of: text and quiet text need 4.5:1
 *  against the background, the accent 3:1 (it carries the focus ring and lights
 *  what is on). */
export function contrastWarnings(colours: Record<string, string>): string[] {
  const need: [string, number, string][] = [
    ['--fg', 4.5, 'Text'],
    ['--muted', 4.5, 'Quiet text'],
    ['--accent', 3, 'Actions'],
  ];
  return need
    .filter(([name, min]) => contrast(colours[name], colours['--bg']) < min)
    .map(([, min, label]) => `${label} is under ${min}:1 against the background`);
}
