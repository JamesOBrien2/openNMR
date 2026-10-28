import { expect, it } from 'vitest';
import { pressPan, pressZoom, viewAction, wheelAction } from './tools';

const turn = (deltaY: number, deltaX = 0, held: { shiftKey?: boolean; metaKey?: boolean } = {}) =>
  wheelAction({ deltaX, deltaY, shiftKey: false, metaKey: false, ...held });

it('reads a turn of the wheel', () => {
  expect(turn(100).kind).toBe('plain');
  expect(turn(100, 0, { metaKey: true }).kind).toBe('zoom');
  expect(turn(100, 0, { shiftKey: true }).kind).toBe('pan');
  // A mouse sends Shift as a turn along the other axis.
  expect(turn(0, 100)).toMatchObject({ kind: 'pan', amount: 100 });
  // Cmd with Shift zooms a tenth as fast.
  const fine = Math.log(turn(100, 0, { metaKey: true, shiftKey: true }).factor);
  expect(fine).toBeCloseTo(Math.log(turn(100, 0, { metaKey: true }).factor) / 10);
});

const key = (key: string, held: { shiftKey?: boolean; metaKey?: boolean } = {}) => ({
  key,
  shiftKey: false,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  ...held,
});

it('reads the keys every plot shares', () => {
  expect(viewAction(key('z'))).toBe('zoom');
  expect(viewAction(key('Z', { shiftKey: true }))).toBe('zoomOut');
  expect(viewAction(key('p'))).toBe('pan');
  expect(viewAction(key('F'))).toBe('full');
  expect(viewAction(key('='))).toBe('larger');
  expect(viewAction(key('ArrowLeft', { shiftKey: true }))).toBe('back');
  expect(viewAction(key('Escape'))).toBe('pointer');
});

it('leaves a plot its own keys, and any held with a modifier', () => {
  // Shift+P phases a spectrum; I integrates one.
  expect(viewAction(key('P', { shiftKey: true }))).toBeUndefined();
  expect(viewAction(key('i'))).toBeUndefined();
  expect(viewAction(key('z', { metaKey: true }))).toBeUndefined();
  expect(viewAction(key('ArrowLeft'))).toBeUndefined();
});

it('turns Z and P pressed again into the next direction', () => {
  expect(pressZoom('pointer', 'xy')).toEqual(['zoom', 'x']);
  expect(pressZoom('zoom', 'x')).toEqual(['zoom', 'y']);
  expect(pressPan('zoom', 'y')).toEqual(['pan', 'xy']);
  expect(pressPan('pan', 'xy')).toEqual(['pan', 'x']);
});
