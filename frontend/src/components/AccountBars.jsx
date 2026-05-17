import { chartPalette } from "../config";
import { formatMoney } from "../utils/formatters";

export function AccountBars({ rows, total, currency = "IDR" }) {
  return (
    <div className="bars">
      {rows.map((row, index) => {
        const percent = total ? Math.round((row.total / total) * 100) : 0;
        return (
          <div className="bar-item" key={row.name}>
            <div><strong>{row.name}</strong><span>{formatMoney(row.total, currency)}</span></div>
            <div className="track"><span style={{ width: `${percent}%`, background: chartPalette[index % chartPalette.length] }} /></div>
            <small>{percent}% of selected month</small>
          </div>
        );
      })}
    </div>
  );
}
