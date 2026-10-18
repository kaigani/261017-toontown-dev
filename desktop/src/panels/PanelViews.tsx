function Placeholder({ title, detail }: { title: string; detail: string }) {
  return (
    <div className="panel-placeholder">
      <h2>{title}</h2>
      <p>{detail}</p>
    </div>
  );
}

export function DrawingPanel() {
  return (
    <Placeholder title="Drawing view" detail="Placeholder. The interactive drawing view arrives in M4." />
  );
}

export function TimelinePanel() {
  return <Placeholder title="Timeline" detail="Placeholder. The Timeline arrives in M5." />;
}

export function PropertiesPanel() {
  // 01 §5.2: the Properties panel shows an explicit "no properties" state when nothing is selected.
  return (
    <div className="panel-placeholder">
      <p className="no-properties">No properties</p>
      <p>Nothing is selected.</p>
    </div>
  );
}
