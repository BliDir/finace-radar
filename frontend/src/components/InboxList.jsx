import { Mail } from "lucide-react";

import { formatMoney } from "../utils/formatters";

export function InboxList({ rows, analyses, financeOnly = false }) {
  if (!rows.length) {
    return (
      <div className="empty-state">
        <Mail size={24} />
        <strong>{financeOnly ? "No finance emails" : "No Gmail messages loaded"}</strong>
        <span>
          {financeOnly
            ? "Switch to All emails or analyze your inbox to detect transactions."
            : "Connect Gmail and read your inbox to load receipts, invoices, and card alerts."}
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
              <strong>{row.subject || "(No subject)"}</strong>
              <span>{row.from || "Unknown sender"}</span>
            </div>
            <p>{row.snippet || "No preview available"}</p>
            <div className="analysis-result">
              <span className={`pill ${analysis?.isFinance ? `direction-${analysis.direction}` : "subtle"}`}>
                {analysis ? (analysis.isFinance ? analysis.direction : "non-finance") : "not analyzed"}
              </span>
              {analysis?.isFinance && (
                <>
                  <span>
                    {analysis.amount ? formatMoney(Number(analysis.amount), analysis.currency ?? "IDR") : "No amount"}
                  </span>
                  <span>{analysis.from ?? "Unknown"} -&gt; {analysis.to ?? "Unknown"}</span>
                  <span>{analysis.account ?? "No account"}</span>
                </>
              )}
            </div>
            <time>{row.date}</time>
            <details className="email-body-preview">
              <summary>Body text ({row.body.length} chars)</summary>
              <p>{row.body || "No body text extracted"}</p>
            </details>
          </article>
        );
      })}
    </div>
  );
}
