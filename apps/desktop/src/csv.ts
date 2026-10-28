import type { DocumentInfo } from './api';

/** RFC 4180 field: quoted when it contains a comma, quote or line break. */
const field = (value: string | number) => {
  const s = String(value);
  return /[",\r\n]/.test(s) ? `"${s.replaceAll('"', '""')}"` : s;
};

const table = (header: string[], rows: (string | number)[][]) =>
  [header, ...rows].map((row) => row.map(field).join(',')).join('\n') + '\n';

export const peaksCsv = (doc: DocumentInfo) =>
  table(
    ['spectrum', 'ppm', 'intensity'],
    doc.analysis.peaks.map((ppm, i) => [doc.name, ppm, doc.peakIntensities[i] ?? 0]),
  );

export const integralsCsv = (doc: DocumentInfo) =>
  table(
    ['spectrum', 'from_ppm', 'to_ppm', 'value'],
    doc.analysis.integrals.map(([from, to], i) => [
      doc.name,
      from,
      to,
      doc.integralValues[i] ?? '',
    ]),
  );
