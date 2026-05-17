import { Bell } from "lucide-react";

import { formatMoney } from "../utils/formatters";

export function SubscriptionGrid({ rows, currency = "IDR", t }) {
  if (!rows.length) {
    return (
      <div className="empty-state compact">
        <Bell size={22} />
        <strong>{t.noSubscriptionsDetected}</strong>
        <span>{t.subscriptionsEmpty}</span>
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
            <dt>{t.account}</dt><dd>{row.account}</dd>
            <dt>{t.renews}</dt><dd>{row.nextRenewal ?? t.detected}</dd>
            <dt>{t.email}</dt><dd>{row.source}</dd>
          </dl>
        </article>
      ))}
    </div>
  );
}
