import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import { mount } from 'svelte';
import App from '../App.svelte';
import '../app.css';
import { handle } from './backend';

mockWindows('main');
mockIPC((cmd, payload) => handle(cmd, (payload ?? {}) as Record<string, unknown>), {
  shouldMockEvents: true,
});

const target = document.getElementById('app');
if (!target) throw new Error('Missing #app element');

const fidelity = document.createElement('p');
fidelity.textContent = 'Browser preview · illustrative analysis and fitting';
fidelity.setAttribute('role', 'status');
fidelity.style.cssText =
  'position:fixed;bottom:0.5rem;right:0.75rem;z-index:1000;margin:0;padding:0.25rem 0.5rem;background:var(--bg);color:var(--muted);font:var(--text-label) var(--font);pointer-events:none';
document.body.append(fidelity);

export default mount(App, { target });
