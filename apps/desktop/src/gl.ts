export type Vec4 = [number, number, number, number];

export interface Trace {
  key: number;
  /** Visible range in buffer coordinates: x0, x1, y0, y1. */
  view: Vec4;
  color: Vec4;
  /** Optional scissor box in device pixels (x, y, width, height) with the origin
   * at the bottom left, to keep a tall peak inside its own row of a stack. */
  clip?: Vec4;
}

const VERTEX = `#version 300 es
in vec2 point;
uniform vec4 view; // x0, x1, y0, y1
void main() {
  vec2 t = (point - view.xz) / (view.yw - view.xz);
  gl_Position = vec4(t * 2.0 - 1.0, 0.0, 1.0);
}`;

const FRAGMENT = `#version 300 es
precision mediump float;
uniform vec4 color;
out vec4 fragColor;
void main() {
  fragColor = color;
}`;

/** Draws decimated traces as WebGL2 line strips, one vertex buffer per trace. */
export class TraceRenderer {
  private readonly gl: WebGL2RenderingContext;
  private readonly view: WebGLUniformLocation | null;
  private readonly color: WebGLUniformLocation | null;
  private readonly buffers = new Map<
    number,
    { buffer: WebGLBuffer; count: number; segments: boolean }
  >();

  constructor(canvas: HTMLCanvasElement) {
    // Keeping the drawing buffer lets PNG export copy the canvas after a frame.
    const gl = canvas.getContext('webgl2', { preserveDrawingBuffer: true });
    if (!gl)
      throw new Error('This computer does not support WebGL2, which is needed to draw spectra.');
    this.gl = gl;
    const program = link(gl);
    gl.useProgram(program);
    this.view = gl.getUniformLocation(program, 'view');
    this.color = gl.getUniformLocation(program, 'color');
    gl.enableVertexAttribArray(0);
  }

  /** `segments` draws each two points as a line of their own, as contours
   *  come, rather than one line through them all. */
  setData(key: number, xy: Float32Array, segments = false) {
    const gl = this.gl;
    let entry = this.buffers.get(key);
    if (!entry) {
      entry = { buffer: gl.createBuffer(), count: 0, segments };
      this.buffers.set(key, entry);
    }
    entry.segments = segments;
    gl.bindBuffer(gl.ARRAY_BUFFER, entry.buffer);
    gl.bufferData(gl.ARRAY_BUFFER, xy, gl.DYNAMIC_DRAW);
    entry.count = xy.length / 2;
  }

  draw(traces: Trace[]) {
    const gl = this.gl;
    gl.viewport(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight);
    // Clearing obeys the scissor box, so it has to be off for the clear itself.
    gl.disable(gl.SCISSOR_TEST);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    for (const trace of traces) {
      const entry = this.buffers.get(trace.key);
      if (!entry) continue;
      if (trace.clip) {
        gl.enable(gl.SCISSOR_TEST);
        gl.scissor(...trace.clip);
      } else {
        gl.disable(gl.SCISSOR_TEST);
      }
      gl.bindBuffer(gl.ARRAY_BUFFER, entry.buffer);
      gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
      gl.uniform4fv(this.view, trace.view);
      gl.uniform4fv(this.color, trace.color);
      gl.drawArrays(entry.segments ? gl.LINES : gl.LINE_STRIP, 0, entry.count);
    }
    gl.disable(gl.SCISSOR_TEST);
  }
}

/** `#rrggbb` as normalised RGBA. */
export function hexColor(hex: string): Vec4 {
  const n = Number.parseInt(hex.slice(1), 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255, 1];
}

function link(gl: WebGL2RenderingContext): WebGLProgram {
  const program = gl.createProgram();
  gl.attachShader(program, compile(gl, gl.VERTEX_SHADER, VERTEX));
  gl.attachShader(program, compile(gl, gl.FRAGMENT_SHADER, FRAGMENT));
  gl.bindAttribLocation(program, 0, 'point');
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    throw new Error(`WebGL program failed to link: ${gl.getProgramInfoLog(program)}`);
  }
  return program;
}

function compile(gl: WebGL2RenderingContext, type: GLenum, source: string): WebGLShader {
  const shader = gl.createShader(type);
  if (!shader) throw new Error('WebGL could not create a shader');
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    throw new Error(`WebGL shader failed to compile: ${gl.getShaderInfoLog(shader)}`);
  }
  return shader;
}
