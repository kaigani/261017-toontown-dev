import 'dockview-react/dist/styles/dockview.css';
import './styles.css';

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App } from './App';
import { createDefaultHostApi, HostProvider } from './ipc';

const root = document.getElementById('root');
if (!root) throw new Error('index.html has no #root element');

// A floating panel's window loads `index.html?panel=<panelId>` (ruling O-2); the main window has no parameter.
const panel = new URLSearchParams(window.location.search).get('panel');
const floatPanel = panel && /^[A-Za-z][A-Za-z0-9]{0,63}$/.test(panel) ? panel : undefined;

createRoot(root).render(
  <StrictMode>
    <HostProvider api={createDefaultHostApi()}>
      <App {...(floatPanel ? { floatPanel } : {})} />
    </HostProvider>
  </StrictMode>,
);
