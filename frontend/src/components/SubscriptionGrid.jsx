import { Bell } from "lucide-react";

import { formatMoney } from "../utils/formatters";

export function SubscriptionGrid({ rows, currency = "IDR" }) {
  if (!rows.length) {
    return (
      <div className="empty-state compact">
        <Bell size={22} />
        <strong>No subscriptions detected</strong>
        <span>Recurring charges appear here after Gmail messages are parsed.</span>
      </div>
    );
  }

  return (
    <div className="subscriptions">
      {rows.map((row) => (
        <article className="subscription" key={row.merchant}>
          <header>
            <div>
              <strong>{row.merchant}</strong>
              <span>{row.category}</span>
            </div>
            <b>{formatMoney(row.amount, row.currency ?? currency)}</b>
          </header>
          <dl>
            <dt>Account</dt><dd>{row.account}</dd>
            <dt>Renews</dt><dd>{row.nextRenewal ?? "Detected"}</dd>
            <dt>Email</dt><dd>{row.source}</dd>
          </dl>
        </article>
      ))}
    </div>
  );
}
