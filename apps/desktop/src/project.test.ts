import { describe, expect, it } from 'vitest';
import type { DocumentInfo } from './api';
import { PAGE_SETUP } from './page';
import {
  bundle,
  isProjectFile,
  parseProject,
  shortPaths,
  structureFile,
  toProject,
  withExtension,
} from './project';

const analysis = { integrals: [], reference: null, peaks: [1.2] };
const docs = [
  { id: 4, path: '/data/a/1', operations: [{ op: 'ft' }], analysis },
  { id: 9, path: '/data/b/2', operations: [], analysis },
] as unknown as DocumentInfo[];

describe('projects', () => {
  it('keeps manual baseline tuning and legacy operations when reopening', () => {
    for (const mode of [{}, { auto_filter: false }]) {
      const operations = [
        { op: 'baseline_whittaker' as const, lambda: 32768, filter_hz: 2.11, ...mode },
      ];
      const source = [{ ...docs[0], operations }];
      const reopened = parseProject(JSON.stringify(toProject(source, 4, [], 'stacked')));
      expect(reopened.documents[0].operations).toEqual(operations);
    }
  });
  it('stores documents by position and round-trips through JSON', () => {
    const project = toProject(docs, 9, [4], 'stacked');
    expect(project.active).toBe(1);
    expect(project.overlay).toEqual([0]);
    expect(project.documents[0]).toEqual({
      path: '/data/a/1',
      operations: [{ op: 'ft' }],
      analysis,
    });
    expect(parseProject(JSON.stringify(project))).toEqual(project);
  });

  it('points into its archive and keeps where the data came from', () => {
    const project = bundle(toProject(docs, 9, [4], 'stacked'), [
      { path: 'spectra/0/a/1', name: 'Start', structure: 'spectra/0/structure.png' },
      { path: 'spectra/1/b/2', color: '#ff0000' },
    ]);
    expect(project.version).toBe(2);
    expect(project.documents[0]).toMatchObject({
      path: 'spectra/0/a/1',
      source: '/data/a/1',
      name: 'Start',
      structure: 'spectra/0/structure.png',
    });
    expect(project.documents[1]).toMatchObject({ source: '/data/b/2', color: '#ff0000' });
    expect(parseProject(JSON.stringify(project))).toEqual(project);
    // A project written before the archive still reads.
    expect(parseProject(JSON.stringify({ ...project, version: 1 })).version).toBe(1);
  });

  it('keeps the page it is printed on', () => {
    const project = toProject(docs, 9, [4], 'stacked', [], undefined, PAGE_SETUP);
    expect(project.page).toEqual(PAGE_SETUP);
    expect(parseProject(JSON.stringify(project))).toEqual(project);
  });

  it('names a structure for what it is', () => {
    expect(structureFile(0, 'data:image/png;base64,AAAA')).toBe('spectra/0/structure.png');
    expect(structureFile(3, 'data:image/jpeg;base64,AAAA')).toBe('spectra/3/structure.jpg');
    expect(structureFile(1, 'data:application/pdf;base64,AA')).toBe('spectra/1/structure.pdf');
  });

  it('stores a view by position, leaving out spectra that have been closed', () => {
    const views = [
      { id: 1, name: 'Stack of 2 spectra', ids: [9, 4], stack: 'stacked' as const },
      { id: 2, name: 'Superimposed of 2 spectra', ids: [4, 7], stack: 'overlay' as const },
    ];
    const project = toProject(docs, 9, [4], 'stacked', views, 1);
    // The page that was open comes back with it.
    expect(project.view).toBe(1);
    expect(toProject(docs, 9, [4], 'stacked', views).view).toBeNull();
    expect(project.views).toEqual([
      { name: 'Stack of 2 spectra', documents: [1, 0], stacked: 'stacked' },
      // 7 was closed before saving, so the view keeps what is left of it.
      { name: 'Superimposed of 2 spectra', documents: [0], stacked: 'overlay' },
    ]);
    expect(parseProject(JSON.stringify(project))).toEqual(project);
  });

  it('explains what is wrong with other files', () => {
    expect(() => parseProject('not json')).toThrow('not valid JSON');
    expect(() => parseProject('{"format":"other"}')).toThrow('not an openNMR project');
  });
});

describe('isProjectFile', () => {
  it('knows projects by either extension, and folders are not projects', () => {
    expect(isProjectFile('/a/prova.opennmr')).toBe(true);
    expect(isProjectFile('/a/prova.opennmr.json')).toBe(true);
    expect(isProjectFile('/a/L1C-Cl_00/1')).toBe(false);
  });
});

describe('withExtension', () => {
  it('saves a project as .opennmr whatever was typed', () => {
    expect(withExtension('/a/prova', 'opennmr')).toBe('/a/prova.opennmr');
    expect(withExtension('/a/prova.opennmr', 'opennmr')).toBe('/a/prova.opennmr');
    expect(withExtension('/a/prova.opennmr.json', 'opennmr')).toBe('/a/prova.opennmr');
    expect(withExtension('/a/prova.json', 'opennmr')).toBe('/a/prova.opennmr');
    expect(withExtension('/a/plot', 'svg')).toBe('/a/plot.svg');
  });
});

describe('shortPaths', () => {
  it('keeps as many folders as it takes to tell paths apart', () => {
    expect(shortPaths(['/d/DMSO/L1C/exp_00', '/d/CDCl3/L1C/exp_00', '/x/prova.opennmr'])).toEqual([
      'DMSO / L1C / exp_00',
      'CDCl3 / L1C / exp_00',
      'x / prova.opennmr',
    ]);
  });
});

it('keeps fitted analysis and method choices without changing legacy defaults', () => {
  const fitted = {
    ...docs[0],
    analysis: {
      ...docs[0].analysis,
      peakMethod: 'deconvolution' as const,
      integrationMethod: 'fitted' as const,
      peakFit: {
        source: 'native-model',
        peaks: [{ ppm: 1.2, height: 10, widthHz: 1.5, area: 0.06, shape: 0.5 }],
        regions: [{ from: 1.4, to: 1.1, status: 'converged' as const, rms: 0.002 }],
      },
    },
  };
  const project = parseProject(
    JSON.stringify(toProject([fitted, docs[1]], fitted.id, [], 'overlay')),
  );
  expect(project.documents[0].analysis).toEqual(fitted.analysis);
  expect(project.documents[1].analysis.peakMethod).toBeUndefined();
  expect(project.documents[1].analysis.integrationMethod).toBeUndefined();
});
