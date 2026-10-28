import { expect, it } from 'vitest';
import { paintScale, SvgPainter } from './painter';

it('writes lines, rectangles and escaped text as SVG', () => {
  const p = new SvgPainter();
  p.line([0, 1.04, 10.26, 2], '#123456', [4, 2]);
  p.line([5, 5], '#000');
  p.rect(1, 2, 3, 4, 'red', 0.1);
  p.text(5, 6, 'a < b & "c"', { color: '#fff', size: 11, align: 'center', vertical: true });
  const svg = p.toString(100, 50, '#ffffff');
  expect(svg).toContain(
    '<polyline fill="none" stroke="#123456" stroke-dasharray="4 2" points="0,1 10.3,2"/>',
  );
  expect(svg.match(/<polyline/g)).toHaveLength(1);
  expect(svg).toContain('fill-opacity="0.1"');
  expect(svg).toContain('text-anchor="middle"');
  expect(svg).toContain('transform="rotate(-90 5 6)">a &lt; b &amp; &quot;c&quot;</text>');
  expect(svg.startsWith('<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50"')).toBe(
    true,
  );
});

it('letters a ppm scale, highest shift on the left, and a page its title', () => {
  const scale = (title?: string) => {
    const p = new SvgPainter();
    paintScale(p, { from: 10, to: 0, length: 600, top: 0, color: '#000', size: 11, gap: 8, title });
    return p.toString(600, 40, '#fff');
  };
  const screen = scale();
  // 600 px at one number per 60 px: every whole ppm, 10 at the left edge.
  expect(screen.match(/<text/g)).toHaveLength(11);
  expect(screen).toMatch(/<text x="0.5" [^>]*>10<\/text>/);
  expect(screen).toMatch(/<text x="600.5" [^>]*>0<\/text>/);
  expect(scale('ppm')).toContain('>ppm</text>');
});

it('stands a ppm scale on its side, the first shift at the top', () => {
  const p = new SvgPainter();
  paintScale(p, {
    from: 0,
    to: 100,
    length: 300,
    top: 0,
    color: '#000',
    size: 11,
    gap: 8,
    vertical: true,
  });
  const svg = p.toString(40, 300, '#fff');
  // 300 px at one number per 30 px: every 10 ppm, 0 at the top.
  expect(svg.match(/<text/g)).toHaveLength(11);
  expect(svg).toMatch(/<text x="8" y="0.5" [^>]*>0<\/text>/);
  expect(svg).toMatch(/<text x="8" y="300.5" [^>]*>100<\/text>/);
});
