import { formatMoney, formatSignal } from "../utils/formatters";

export function TransactionTable({ rows, currency = "IDR" }) {
  return (
    <div className="table-scroll">
      <table>
        <thead>
          <tr>
            <th>Date</th>
            <th>Type</th>
            <th>From</th>
            <th>To</th>
            <th>Category</th>
            <th>Account</th>
            <th>Signal</th>
            <th>Email source</th>
            <th>Amount</th>
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
              <td><span className="pill subtle">{formatSignal(row.accountConfidence)}</span></td>
              <td>{row.source}</td>
              <td>{formatMoney(row.amount, row.currency ?? currency)}</td>
            </tr>
          ))}
          {!rows.length && (
            <tr>
              <td colSpan="9">No account-backed finance transactions yet.</td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
