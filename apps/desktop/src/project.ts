import type { Analysis, CrossPeak, DocumentInfo, Operation } from './api';
import type { StackMode } from './traces';
import type { PageSetup } from './page';

/** One spectrum of a project. Version 2 is written inside a `.opennmr` archive
 * beside the data, and `path` is where that data is in the archive; version 1
 * held nothing but a path on the disk it was saved on. */
export interface ProjectDocument {
  path: string;
  operations: Operation[];
  analysis: Analysis;
  /** Where the data was read from when it was saved, for the record. */
  source?: string;
  /** The name given in the list, and the colour chosen for the trace. */
  name?: string;
  color?: string;
  /** The structure drawing, by its place in the archive; the drawing itself, as
   * a data URL, in a session kept for recovery. */
  structure?: string;
}

/** A saved session: its spectra with their processing and analysis. */
export interface Project {
  format: 'opennmr-project';
  version: 1 | 2;
  documents: ProjectDocument[];
  /** Index into `documents`. */
  active: number;
  overlay: number[];
  /** Version 1 wrote a boolean here before cascades existed. */
  stacked: StackMode | boolean;
  /** Saved arrangements, holding positions in `documents` rather than ids,
   * which belong to a session. Files written before views have none. */
  views?: { name: string; documents: number[]; stacked: StackMode }[];
  /** Which view was on screen, as a position in `views`; null for one spectrum
   * on its own. Files written before views have none. */
  view?: number | null;
  /** 2D experiments, drawn as contours from the parameters they were recorded
   * with. Files written before 2D have none, and earlier versions of the app
   * pass over them. */
  documents2d?: {
    path: string;
    source?: string;
    peaks?: CrossPeak[];
    /** What is chosen for F2's side and F1's: a spectrum as a position in
     * `documents`, `own` for the 2D's projection, null for automatic. */
    sides?: (number | 'own' | null)[];
  }[];
  /** The page it is printed on. Files written before pages have none. */
  page?: PageSetup;
  /** Only in a session kept for recovery: the project file it was working on,
   * and the temporary folder that file's data had been unpacked into. */
  session?: { projectPath?: string; root?: string };
}

export function toProject(
  docs: DocumentInfo[],
  activeId: number | undefined,
  overlay: number[],
  stacked: StackMode,
  views: { name: string; ids: number[]; stack: StackMode }[] = [],
  showing?: number,
  page?: PageSetup,
): Project {
  const index = (id: number) => docs.findIndex((d) => d.id === id);
  return {
    format: 'opennmr-project',
    version: 2,
    documents: docs.map(({ path, operations, analysis }) => ({ path, operations, analysis })),
    active: activeId === undefined ? 0 : Math.max(0, index(activeId)),
    overlay: overlay.map(index).filter((i) => i >= 0),
    stacked,
    views: views.map((view) => ({
      name: view.name,
      documents: view.ids.map(index).filter((i) => i >= 0),
      stacked: view.stack,
    })),
    view: showing ?? null,
    page,
  };
}

/** `project` with its documents pointing into the archive they are saved in,
 * each carrying what the list shows of it. `entries` is by position. */
export const bundle = (
  project: Project,
  entries: Omit<ProjectDocument, 'operations' | 'analysis' | 'source'>[],
): Project => ({
  ...project,
  documents: project.documents.map((doc, k) => ({ ...doc, source: doc.path, ...entries[k] })),
});

/** Where in the archive the structure of document `k` is kept, named for what
 * the drawing is: `data:image/png;base64,…` is a PNG. */
export function structureFile(k: number, dataUrl: string): string {
  const type = /^data:([^;,]+)/.exec(dataUrl)?.[1] ?? '';
  const ext =
    {
      'image/jpeg': 'jpg',
      'image/svg+xml': 'svg',
      'application/pdf': 'pdf',
      'chemical/x-mdl-molfile': 'mol',
    }[type] ??
    type.split('/')[1] ??
    'bin';
  return `spectra/${k}/structure.${ext}`;
}

/** `.opennmr` is what is written; `.json` is what earlier versions wrote, and a
 * PDF exported here carries its project inside. */
export const PROJECT_EXTENSIONS = ['opennmr', 'json', 'pdf'];

export const isProjectFile = (path: string) =>
  PROJECT_EXTENSIONS.some((ext) => path.toLowerCase().endsWith(`.${ext}`));

/**
 * `path` ending in `.extension`, whatever the save dialog handed back: a dialog
 * keeps what was typed, so a name typed with `.json` came back as a project
 * called `.opennmr.json`. A `.json` a project name was typed with is replaced.
 */
export function withExtension(path: string, extension: string): string {
  const lower = path.toLowerCase();
  if (lower.endsWith(`.${extension}`)) return path;
  const base =
    extension === PROJECT_EXTENSIONS[0] && lower.endsWith('.json') ? path.slice(0, -5) : path;
  return base.toLowerCase().endsWith(`.${extension}`) ? base : `${base}.${extension}`;
}

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null;

/** Checks the overall shape; operations and analysis are validated when applied. */
export function parseProject(text: string): Project {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new Error('This file is not valid JSON, so it cannot be an openNMR project.');
  }
  const valid =
    isRecord(value) &&
    value.format === 'opennmr-project' &&
    (value.version === 1 || value.version === 2) &&
    Array.isArray(value.documents) &&
    value.documents.every(
      (d) =>
        isRecord(d) &&
        typeof d.path === 'string' &&
        Array.isArray(d.operations) &&
        isRecord(d.analysis),
    );
  if (!valid) throw new Error('This file is not an openNMR project.');
  return value as unknown as Project;
}

/**
 * Each path cut to the fewest trailing folders, two at least, that tell it
 * apart from the others: the same experiment names recur under every solvent
 * and sample, so a fixed number of folders showed different paths identically.
 */
export function shortPaths(paths: string[]): string[] {
  const parts = paths.map((p) => p.split(/[\\/]/).filter(Boolean));
  return parts.map((mine, i) => {
    for (let n = 2; n < mine.length; n++) {
      const tail = mine.slice(-n).join('/');
      const clash = parts.some((other, j) => j !== i && other.slice(-n).join('/') === tail);
      if (!clash) return mine.slice(-n).join(' / ');
    }
    return mine.join(' / ');
  });
}
