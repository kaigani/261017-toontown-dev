import 'dockview-react/dist/styles/dockview.css';
import './styles.css';

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App } from './App';
import { createDefaultHostApi, HostProvider } from './ipc';

const root = document.getElementById('root');
if (!root) throw new Error('index.html has no #root element');

createRoot(root).render(
  <StrictMode>
    <HostProvider api={createDefaultHostApi()}>
      <App />
    </HostProvider>
  </StrictMode>,
);
