import type { Molecule } from './api';

/** A molfile is kept like any other structure, as a data URL, so the project
 * writes, reads and copies it with the pictures. */
export const MOLFILE_TYPE = 'chemical/x-mdl-molfile';

/** The counts line, the fourth, names the version. */
export const looksLikeMolfile = (text: string) =>
  /V[23]000/.test(text.split(/\r\n|\r|\n/)[3] ?? '');

export function molfileUrl(text: string): string {
  let binary = '';
  new TextEncoder().encode(text).forEach((b) => (binary += String.fromCharCode(b)));
  return `data:${MOLFILE_TYPE};base64,${btoa(binary)}`;
}

/** The text of a molfile kept by `molfileUrl`, or undefined for any other structure. */
export function molfileText(url: string): string | undefined {
  if (!url.startsWith(`data:${MOLFILE_TYPE};base64,`)) return undefined;
  const binary = atob(url.slice(url.indexOf(',') + 1));
  return new TextDecoder().decode(Uint8Array.from(binary, (c) => c.charCodeAt(0)));
}

export interface Drawing {
  /** In bond lengths, y down, so the caller only scales. */
  points: { x: number; y: number }[];
  /** The typical bond length in the file's own units, which the sizes are set in. */
  unit: number;
}

/** Where each atom goes: in bond lengths, y flipped (a molfile's is up). */
export function layout(m: Molecule): Drawing {
  const lengths = m.bonds.map((b) =>
    Math.hypot(m.atoms[b.a].x - m.atoms[b.b].x, m.atoms[b.a].y - m.atoms[b.b].y),
  );
  // A molecule of one atom has no bond to measure by.
  const unit = lengths.length ? lengths.reduce((s, l) => s + l, 0) / lengths.length : 1;
  return { points: m.atoms.map((a) => ({ x: a.x / unit, y: -a.y / unit })), unit };
}
