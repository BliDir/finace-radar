export function Panel({ id, title, subtitle, children, className = "" }) {
  return (
    <section id={id} className={`panel ${className}`}>
      <header className="panel-header">
        <div>
          <h2>{title}</h2>
          <p>{subtitle}</p>
        </div>
      </header>
      {children}
    </section>
  );
}
