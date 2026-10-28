import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import {
  fetchBaselineTrace,
  fetchContours,
  fetchIntegralCurve,
  fetchPhaseTrace,
  fetchProjection,
  fetchTrace,
  readBytes,
} from './api';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

beforeEach(() => vi.mocked(invoke).mockReset());

const values = [0, -125000000, 123, 458700000];
const bytesOf = (values: number[]) => {
  const bytes = new ArrayBuffer(values.length * 4);
  const view = new DataView(bytes);
  values.forEach((value, i) => view.setFloat32(i * 4, value, true));
  return bytes;
};

for (const transport of ['binary', 'postMessage'] as const) {
  describe(`${transport} IPC response`, () => {
    const reply = (bytes: ArrayBuffer) =>
      vi
        .mocked(invoke)
        .mockResolvedValue(transport === 'binary' ? bytes : Array.from(new Uint8Array(bytes)));

    it.each([
      ['trace', () => fetchTrace(1, 0, 1000, 300)],
      ['phase', () => fetchPhaseTrace(1, { p0: 0, p1: 0, pivot: 0 }, 0, 1000, 300)],
      ['baseline', () => fetchBaselineTrace(1, null, 0, 1000, 300)],
      ['integral', () => fetchIntegralCurve(1, 8, 7, 300)],
      ['projection', () => fetchProjection(1, 0, 0, 1000, 300)],
    ])('decodes %s bytes into the same signed float values', async (_, fetch) => {
      reply(bytesOf(values));
      expect(Array.from(await fetch())).toEqual(values);
    });

    it('decodes contour lengths and coordinates before splitting levels', async () => {
      reply(bytesOf([4, 4, 0, 1, 2, 3, 4, 5, 6, 7]));
      expect((await fetchContours(1, [1, 2])).map((xy) => Array.from(xy))).toEqual([
        [0, 1, 2, 3],
        [4, 5, 6, 7],
      ]);
    });

    it('preserves file bytes', async () => {
      reply(new Uint8Array([0, 10, 128, 255]).buffer);
      expect(Array.from(new Uint8Array(await readBytes('drawing.png')))).toEqual([0, 10, 128, 255]);
    });

    it('rejects an incomplete float instead of rendering corrupt coordinates', async () => {
      reply(new Uint8Array([0, 1, 2]).buffer);
      await expect(fetchTrace(1, 0, 10, 10)).rejects.toThrow();
    });
  });
}
