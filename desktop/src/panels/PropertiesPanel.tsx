// The Properties panel (01 §5.2 `properties`; M4 C4.4a): pages for the active tool's options and the current pen (the
// Pen panel's content, 02 §6.1). Choosing a page changes nothing but what the panel shows (`navigate`).
import { useState } from 'react';

import { PenPanel } from './PenPanel';
import { ToolOptionsPage } from './ToolOptionsPage';

const PAGES = [
  { id: 'toolOptions', label: 'Tool options' },
  { id: 'pen', label: 'Pen' },
] as const;

type Page = (typeof PAGES)[number]['id'];

export function PropertiesPanel() {
  const [page, setPage] = useState<Page>('toolOptions');
  return (
    <div className="properties-panel" data-tt-role="panel" data-tt-id="properties" data-tt-label="Properties">
      <div className="tabs" role="tablist" aria-label="Properties pages">
        {PAGES.map((p) => (
          <button
            key={p.id}
            type="button"
            role="tab"
            aria-selected={page === p.id}
            data-tt-role="tab"
            data-tt-id={`properties.page.${p.id}`}
            data-tt-gui-only="navigate"
            onClick={() => {
              setPage(p.id);
            }}
          >
            {p.label}
          </button>
        ))}
      </div>
      <div role="tabpanel">{page === 'toolOptions' ? <ToolOptionsPage /> : <PenPanel />}</div>
    </div>
  );
}
