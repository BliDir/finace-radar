import { formatMoney } from "../utils/formatters";

export function TransactionTable({ rows, currency = "IDR", t }) {
  return (
    <div className="table-scroll">
      <table>
        <thead>
          <tr>
            <th>{t.date}</th>
            <th>{t.type}</th>
            <th>{t.from}</th>
            <th>{t.to}</th>
            <th>{t.category}</th>
            <th>{t.account}</th>
            <th>Signal</th>
            <th>{t.emailSource}</th>
            <th>{t.amount}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.id}>
              <td>{row.date}</td>
              <td><span className={`pill direction-${row.direction}`}>{row.direction}</span></td>
              <td><strong>{row.fromParty}</strong></td>
              <td>{row.toParty}</td>
              <td><span className="pill">{row.category}</span></td>
              <td>{row.account}</td>
              <td><span className="pill subtle">{formatSignal(row.accountConfidence, t)}</span></td>
              <td>{row.source}</td>
              <td>{formatMoney(row.amount, row.currency ?? currency)}</td>
            </tr>
          ))}
          {!rows.length && (
            <tr>
              <td colSpan="9">{t.noTransactions}</td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}

function formatSignal(confidence, t) {
  if (confidence === "high") return t.accountMatch;
  if (confidence === "low") return t.senderFallback;
  return t.institutionMatch;
}
