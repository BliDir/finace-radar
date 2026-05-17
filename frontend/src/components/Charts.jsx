import { chartPalette } from "../config";
import { formatMoney } from "../utils/formatters";

export function TrendChart({ months, trendByMonth, selectedMonth, currency = "IDR", t }) {
  const width = 820;
  const height = 270;
  const pad = 34;
  const totals = months.map((month) => trendByMonth.get(month) ?? 0);
  const max = Math.max(...totals, 1);
  const points = totals.map((total, index) => ({
    x: pad + ((width - pad * 2) / Math.max(months.length - 1, 1)) * index,
    y: height - pad - (total / max) * (height - pad * 2),
    total,
  }));
  const line = points.map((point) => `${point.x},${point.y}`).join(" ");
  const area = `${pad},${height - pad} ${line} ${width - pad},${height - pad}`;

  return (
    <svg className="chart" viewBox={`0 0 ${width} ${height}`} role="img" aria-label={t.chartExpenseTrend}>
      <defs>
        <linearGradient id="trendFill" x1="0" x2="0" y1="0" y2="1">
          <stop offset="0%" stopColor="#0f9f94" stopOpacity="0.22" />
          <stop offset="100%" stopColor="#0f9f94" stopOpacity="0.02" />
        </linearGradient>
      </defs>
      {[0, 1, 2, 3].map((lineIndex) => (
        <line key={lineIndex} x1={pad} x2={width - pad} y1={pad + lineIndex * 58} y2={pad + lineIndex * 58} stroke="#d8e8fb" />
      ))}
      <polygon points={area} fill="url(#trendFill)" />
      <polyline points={line} fill="none" stroke="#0f9f94" strokeWidth="5" strokeLinecap="round" strokeLinejoin="round" />
      {points.map((point, index) => (
        <g key={months[index]}>
          <circle cx={point.x} cy={point.y} r={months[index] === selectedMonth ? 8 : 5} fill={months[index] === selectedMonth ? "#58d68d" : "#0f9f94"} />
          <text x={point.x} y={height - 8} textAnchor="middle">{months[index].slice(5)}</text>
          <text x={point.x} y={point.y - 14} textAnchor="middle" className="chart-value">{formatMoney(point.total, currency)}</text>
        </g>
      ))}
    </svg>
  );
}

export function DonutChart({ rows, total, currency = "IDR", t }) {
  let offset = 25;
  return (
    <div className="donut-wrap">
      <svg className="donut" viewBox="0 0 220 220" role="img" aria-label="Category mix">
        <circle cx="110" cy="110" r="78" fill="none" stroke="#eef8f6" strokeWidth="32" />
        {rows.map((row, index) => {
          const part = total ? (row.total / total) * 100 : 0;
          const circle = (
            <circle
              key={row.name}
              cx="110"
              cy="110"
              r="78"
              fill="none"
              stroke={chartPalette[index % chartPalette.length]}
              strokeWidth="32"
              strokeDasharray={`${part} ${100 - part}`}
              strokeDashoffset={offset}
              pathLength="100"
            />
          );
          offset -= part;
          return circle;
        })}
        <text x="110" y="104" textAnchor="middle" className="donut-label">{t.total}</text>
        <text x="110" y="130" textAnchor="middle" className="donut-total">{formatMoney(total, currency)}</text>
      </svg>
      <div className="legend">
        {rows.map((row, index) => (
          <span key={row.name}><i style={{ background: chartPalette[index % chartPalette.length] }} /> {row.name} <b>{formatMoney(row.total, currency)}</b></span>
        ))}
      </div>
    </div>
  );
}
