import 'dockview-react/dist/styles/dockview.css';
import './styles.css';

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App } from './App';
import { watchStartup } from './diagnostics/ipcBench';
import { createDefaultHostApi, HostProvider } from './ipc';

const root = document.getElementById('root');
if (!root) throw new Error('index.html has no #root element');

// A floating panel's window loads `index.html?panel=<panelId>` (ruling O-2); the main window has no parameter.
const panel = new URLSearchParams(window.location.search).get('panel');
const floatPanel = panel && /^[A-Za-z][A-Za-z0-9]{0,63}$/.test(panel) ? panel : undefined;

const api = createDefaultHostApi();
// M0 diagnostics: `--ipc-bench` reports a start-up failure instead of timing out silently (absent from the default build).
if (__TT_M0_DIAGNOSTICS__) watchStartup(api, window);

createRoot(root).render(
  <StrictMode>
    <HostProvider api={api}>
      <App {...(floatPanel ? { floatPanel } : {})} />
    </HostProvider>
  </StrictMode>,
);
