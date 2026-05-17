export function Metric({ title, value, note, icon }) {
  return (
    <article className="metric">
      <div>{icon}</div>
      <span>{title}</span>
      <strong>{value}</strong>
      <small>{note}</small>
    </article>
  );
}
