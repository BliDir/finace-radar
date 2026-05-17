import { Mail } from "lucide-react";

import { formatMoney } from "../utils/formatters";

export function InboxList({ rows, analyses, financeOnly = false, t }) {
  if (!rows.length) {
    return (
      <div className="empty-state">
        <Mail size={24} />
        <strong>{financeOnly ? t.noFinanceEmails : t.noGmailMessagesLoaded}</strong>
        <span>
          {financeOnly
            ? t.switchInboxFilter
            : t.unreadInboxEmpty}
        </span>
      </div>
    );
  }

  return (
    <div className="inbox-list">
      {rows.map((row) => {
        const analysis = analyses.get(row.id);
        return (
          <article className="inbox-message" key={row.id}>
            <div>
              <strong>{row.subject || t.noSubject}</strong>
              <span>{row.from || t.unknownSender}</span>
            </div>
            <p>{row.snippet || t.noPreview}</p>
            <div className="analysis-result">
              <span className={`pill ${analysis?.isFinance ? `direction-${analysis.direction}` : "subtle"}`}>
                {analysis ? (analysis.isFinance ? analysis.direction : "non-finance") : "not analyzed"}
              </span>
              {analysis?.isFinance && (
                <>
                  <span>
                    {analysis.amount ? formatMoney(Number(analysis.amount), analysis.currency ?? "IDR") : t.noAmount}
                  </span>
                  <span>{analysis.from ?? t.unknown} -&gt; {analysis.to ?? t.unknown}</span>
                  <span>{analysis.account ?? t.noAccount}</span>
                </>
              )}
            </div>
            <time>{row.date}</time>
            <details className="email-body-preview">
              <summary>Body text ({row.body.length} chars)</summary>
              <p>{row.body || t.noBodyText}</p>
            </details>
          </article>
        );
      })}
    </div>
  );
}
