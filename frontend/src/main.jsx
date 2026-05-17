import React, { useEffect, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import {
  ArrowDownRight,
  Bell,
  CalendarDays,
  ChevronLeft,
  ChevronRight,
  CreditCard,
  Inbox,
  Landmark,
  LogOut,
  Mail,
  PieChart,
  RefreshCw,
  Search,
  ShieldCheck,
  WalletCards,
} from "lucide-react";
import logoUrl from "./assets/finance-radar-logo-cropped.png";
import "./styles.css";

const GOOGLE_CLIENT_ID = import.meta.env.VITE_GOOGLE_CLIENT_ID;
const GOOGLE_SCOPES = [
  "openid",
  "email",
  "profile",
  "https://www.googleapis.com/auth/gmail.readonly",
].join(" ");
const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? "";

const palette = ["#0f9f94", "#58d68d", "#087c78", "#a8d4d2", "#2f7f89", "#7fa2a6"];
const moneyFormatters = new Map();

function formatMoney(amount, currency = "IDR") {
  const code = (currency || "IDR").toUpperCase();
  if (!moneyFormatters.has(code)) {
    const locale =
      code === "IDR" ? "id-ID" : code === "JPY" ? "ja-JP" : code === "EUR" ? "de-DE" : "en-US";
    const maximumFractionDigits = ["IDR", "JPY", "KRW", "VND"].includes(code) ? 0 : 2;
    moneyFormatters.set(
      code,
      new Intl.NumberFormat(locale, { style: "currency", currency: code, maximumFractionDigits }),
    );
  }
  return moneyFormatters.get(code).format(amount);
}

function dominantCurrency(rows, fallback = "IDR") {
  const counts = new Map();
  rows.forEach((row) => {
    const code = row.currency;
    if (!code) return;
    counts.set(code, (counts.get(code) ?? 0) + 1);
  });
  if (!counts.size) return fallback;
  return [...counts.entries()].sort((a, b) => b[1] - a[1])[0][0];
}

function App() {
  const [selectedMonth, setSelectedMonth] = useState(currentMonth());
  const [query, setQuery] = useState("");
  const [signedInEmail, setSignedInEmail] = useState("");
  const [accessToken, setAccessToken] = useState("");
  const [scanState, setScanState] = useState(
    GOOGLE_CLIENT_ID ? "Loading saved data..." : "Set VITE_GOOGLE_CLIENT_ID to enable Gmail",
  );
  const [isLoading, setIsLoading] = useState(false);
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [inboxMessages, setInboxMessages] = useState([]);
  const [emailAnalyses, setEmailAnalyses] = useState(new Map());
  const [transactions, setTransactions] = useState([]);
  const [trendByMonth, setTrendByMonth] = useState(new Map());
  const [inboxFinanceOnly, setInboxFinanceOnly] = useState(false);
  const [aiConfig, setAiConfig] = useState({ aiProvider: "ai", aiModel: "" });

  const months = useMemo(() => recentMonths(18), []);
  const monthly = useMemo(
    () => transactions.filter((item) => item.date.startsWith(selectedMonth)),
    [selectedMonth, transactions],
  );
  const previous = useMemo(
    () => transactions.filter((item) => item.date.startsWith(shiftMonth(selectedMonth, -1))),
    [selectedMonth, transactions],
  );
  const accounts = useMemo(() => totalsBy(monthly, "account"), [monthly]);
  const categories = useMemo(() => totalsBy(monthly, "category"), [monthly]);
  const subscriptions = useMemo(() => latestSubscriptions(transactions), [transactions]);
  const filtered = monthly.filter((item) =>
    [item.merchant, item.category, item.account, item.source].join(" ").toLowerCase().includes(query.toLowerCase()),
  );
  const filteredInbox = useMemo(() => {
    if (!inboxFinanceOnly) return inboxMessages;
    return inboxMessages.filter((row) => emailAnalyses.get(row.id)?.isFinance);
  }, [inboxMessages, emailAnalyses, inboxFinanceOnly]);

  const displayCurrency = dominantCurrency(transactions);
  const total = sum(monthly);
  const previousTotal = sum(previous);
  const delta = previousTotal ? ((total - previousTotal) / previousTotal) * 100 : 0;
  const selectedIndex = months.indexOf(selectedMonth);
  const hasClientId = Boolean(GOOGLE_CLIENT_ID);
  const aiName = formatAiName(aiConfig);

  useEffect(() => {
    apiFetch("/api/config")
      .then((response) => response.json())
      .then(setAiConfig)
      .catch(() => {});
  }, []);

  useEffect(() => {
    loadDashboard(selectedMonth);
  }, [selectedMonth]);

  useEffect(() => {
    loadSpendingTrend();
  }, []);

  async function loadDashboard(month) {
    setIsLoading(true);
    try {
      const response = await apiFetch(`/api/dashboard?month=${encodeURIComponent(month)}`);
      const data = await response.json();
      applyDashboardData(data);
      const count = (data.transactions ?? []).length;
      setScanState(
        count > 0
          ? `${month}: showing ${count} saved transaction${count === 1 ? "" : "s"} from database`
          : `${month}: no saved data yet — connect Gmail and click Analyze inbox`,
      );
    } catch (error) {
      setScanState(error.message || "Could not load dashboard data");
    } finally {
      setIsLoading(false);
    }
  }

  async function loadSpendingTrend() {
    const from = months[0];
    const to = months[months.length - 1];
    try {
      const response = await apiFetch(
        `/api/spending-trend?from=${encodeURIComponent(from)}&to=${encodeURIComponent(to)}`,
      );
      const data = await response.json();
      setTrendByMonth(new Map((data.points ?? []).map((point) => [point.month, point.spending])));
    } catch {
      setTrendByMonth(new Map());
    }
  }

  async function connectGmail() {
    setIsLoading(true);
    setScanState("Requesting Google Gmail access...");
    try {
      const token = await requestGoogleAccessToken();
      const profile = await fetchGoogleProfile(token).catch(() => ({}));
      setAccessToken(token);
      setSignedInEmail(profile.email ?? "");
      setScanState("Gmail connected. Click Analyze inbox when you want to fetch and classify emails.");
    } catch (error) {
      setScanState(error.message || "Gmail connection failed");
    } finally {
      setIsLoading(false);
    }
  }

  async function analyzeInbox(month = selectedMonth) {
    if (!accessToken) {
      setScanState("Connect Gmail first, then click Analyze inbox.");
      return;
    }
    setIsAnalyzing(true);
    setScanState(`Analyzing ${month} Gmail with ${aiName}...`);
    try {
      const response = await apiFetch("/api/read-inbox", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ month, accessToken, email: signedInEmail }),
      });
      const data = await response.json();
      applyDashboardData(data);
      await loadSpendingTrend();
      const count = (data.transactions ?? []).length;
      setScanState(`${month}: analyzed and saved ${count} transaction${count === 1 ? "" : "s"}`);
    } catch (error) {
      setScanState(error.message || "Inbox analysis failed");
    } finally {
      setIsAnalyzing(false);
    }
  }

  function logout() {
    if (accessToken && window.google?.accounts?.oauth2) {
      window.google.accounts.oauth2.revoke(accessToken);
    }
    setAccessToken("");
    setSignedInEmail("");
    setQuery("");
    setScanState("Signed out. Showing saved database data.");
    loadDashboard(selectedMonth);
    loadSpendingTrend();
  }

  function changeMonth(month) {
    setSelectedMonth(month);
    setQuery("");
  }

  function applyDashboardData(data) {
    const emails = data.emails ?? [];
    const analyses = new Map((data.analyses ?? []).map((analysis) => [analysis.id, analysis]));
    setInboxMessages(emails);
    setEmailAnalyses(analyses);
    setTransactions(data.transactions ?? []);
    if (data.email) setSignedInEmail(data.email);
  }

  return (
    <div className="app">
      <aside className="rail">
        <div className="brand">
          <img className="logo" src={logoUrl} alt="Finance Radar" />
        </div>

        <nav>
          <a href="#overview" className="active"><PieChart size={18} /> Overview</a>
          <a href="#accounts"><WalletCards size={18} /> Accounts</a>
          <a href="#subscriptions"><Bell size={18} /> Subscriptions</a>
          <a href="#transactions"><Inbox size={18} /> Inbox ledger</a>
        </nav>

        <section className="email-card">
          <div className="email-icon"><Inbox size={20} /></div>
          <h2>{signedInEmail ? "Connected Gmail" : "Inbox sync"}</h2>
          {signedInEmail ? (
            <>
              <p className="connected-email"><Mail size={15} /> {signedInEmail}</p>
              <button type="button" onClick={() => analyzeInbox()} disabled={isAnalyzing || isLoading}>
                <RefreshCw size={17} /> {isAnalyzing ? "Analyzing..." : "Analyze inbox"}
              </button>
              <button className="secondary-button" type="button" onClick={logout} disabled={isAnalyzing}>
                <LogOut size={17} /> Sign out
              </button>
            </>
          ) : (
            <div className="login-form">
              <p>Connect Gmail to analyze new messages. Saved data loads automatically.</p>
              <button type="button" onClick={connectGmail} disabled={isLoading || isAnalyzing}>
                <ShieldCheck size={17} /> {isLoading ? "Connecting..." : "Connect Gmail"}
              </button>
            </div>
          )}
          <small>{scanState}</small>
        </section>
      </aside>

      <main>
        <header className="hero">
          <div>
            <span className="kicker">Live Gmail connection</span>
            <h1>{signedInEmail ? "Know where your money goes before the month ends." : "Connect Gmail to chart bank and card activity."}</h1>
            <p>{signedInEmail ? "Track income and payments by detected bank accounts, cards, and wallets from Gmail financial alerts." : "Authorize read-only Gmail access, scan recent finance emails, then use AI to classify income, spending, and payment direction."}</p>
          </div>
          <div className="month-switcher" aria-label="Month selector">
            <button disabled={selectedIndex <= 0 || isLoading} onClick={() => changeMonth(months[selectedIndex - 1])} aria-label="Previous month">
              <ChevronLeft size={18} />
            </button>
            <label>
              <CalendarDays size={17} />
              <input
                type="month"
                value={selectedMonth}
                onChange={(event) => changeMonth(event.target.value)}
                disabled={isLoading}
                aria-label="Email month"
              />
            </label>
            <button disabled={selectedIndex === months.length - 1 || isLoading} onClick={() => changeMonth(months[selectedIndex + 1])} aria-label="Next month">
              <ChevronRight size={18} />
            </button>
          </div>
        </header>

        <section id="overview" className="metrics">
          <Metric title="Monthly spending" value={formatMoney(sum(monthly.filter((item) => item.direction === "spending" || item.direction === "fee")), displayCurrency)} note={`${formatDelta(delta)} total flow vs previous month`} icon={<ArrowDownRight size={20} />} />
          <Metric title="Monthly income" value={formatMoney(sum(monthly.filter((item) => item.direction === "income" || item.direction === "refund")), displayCurrency)} note={`${monthly.filter((item) => item.direction === "income").length} incoming payments`} icon={<Landmark size={20} />} />
          <Metric title="Subscription run-rate" value={formatMoney(sum(monthly.filter((item) => item.recurring)), displayCurrency)} note={`${monthly.filter((item) => item.recurring).length} recurring charges`} icon={<Bell size={20} />} />
          <Metric title="Top account" value={accounts[0]?.name ?? "No data"} note={accounts[0] ? formatMoney(accounts[0].total, displayCurrency) : "Read inbox"} icon={<CreditCard size={20} />} />
        </section>

        <section className="layout-grid">
          <Panel className="span-8" title="Spending trend" subtitle="Monthly spending totals from saved transactions in the database.">
            <TrendChart months={months} trendByMonth={trendByMonth} selectedMonth={selectedMonth} currency={displayCurrency} />
          </Panel>
          <Panel id="accounts" className="span-4" title="Payment accounts" subtitle="Cards, wallets, and banks detected this month.">
            <AccountBars rows={accounts} total={total} currency={displayCurrency} />
          </Panel>
          <Panel className="span-4" title="Category mix" subtitle="Current month distribution.">
            <DonutChart rows={categories} total={total} currency={displayCurrency} />
          </Panel>
          <Panel id="subscriptions" className="span-8" title="Subscriptions" subtitle="Latest recurring charges detected in Gmail.">
            <SubscriptionGrid rows={subscriptions} currency={displayCurrency} />
          </Panel>
          <Panel
            className="span-12"
            title="Read emails"
            subtitle={
              signedInEmail
                ? inboxFinanceOnly
                  ? `${filteredInbox.length} finance messages of ${inboxMessages.length} read for ${selectedMonth}.`
                  : `${inboxMessages.length} messages read for ${selectedMonth} from ${signedInEmail}.`
                : "Connect Gmail to read messages."
            }
          >
            <div className="inbox-toolbar">
              <div className="filter-toggle" role="group" aria-label="Inbox filter">
                <button
                  type="button"
                  className={!inboxFinanceOnly ? "active" : ""}
                  onClick={() => setInboxFinanceOnly(false)}
                  aria-pressed={!inboxFinanceOnly}
                >
                  All emails
                </button>
                <button
                  type="button"
                  className={inboxFinanceOnly ? "active" : ""}
                  onClick={() => setInboxFinanceOnly(true)}
                  aria-pressed={inboxFinanceOnly}
                >
                  Finance emails
                </button>
              </div>
            </div>
            <InboxList rows={filteredInbox} analyses={emailAnalyses} financeOnly={inboxFinanceOnly} />
          </Panel>
          <Panel id="transactions" className="span-12" title="Account ledger" subtitle="Transactions parsed only when a bank, card, or wallet signal is detected.">
            <div className="toolbar">
              <label className="search-box">
                <Search size={18} />
                <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search merchant, account, category" />
              </label>
            </div>
            <TransactionTable rows={filtered} currency={displayCurrency} />
          </Panel>
        </section>
      </main>
    </div>
  );
}

function InboxList({ rows, analyses, financeOnly = false }) {
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
                    {analysis.amount
                      ? formatMoney(Number(analysis.amount), analysis.currency ?? "IDR")
                      : "No amount"}
                  </span>
                  <span>{analysis.from ?? "Unknown"} → {analysis.to ?? "Unknown"}</span>
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

function Metric({ title, value, note, icon }) {
  return (
    <article className="metric">
      <div>{icon}</div>
      <span>{title}</span>
      <strong>{value}</strong>
      <small>{note}</small>
    </article>
  );
}

function Panel({ id, title, subtitle, children, className = "" }) {
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

function TrendChart({ months, trendByMonth, selectedMonth, currency = "IDR" }) {
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
    <svg className="chart" viewBox={`0 0 ${width} ${height}`} role="img" aria-label="Expense trend">
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

function DonutChart({ rows, total, currency = "IDR" }) {
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
              stroke={palette[index % palette.length]}
              strokeWidth="32"
              strokeDasharray={`${part} ${100 - part}`}
              strokeDashoffset={offset}
              pathLength="100"
            />
          );
          offset -= part;
          return circle;
        })}
        <text x="110" y="104" textAnchor="middle" className="donut-label">Total</text>
        <text x="110" y="130" textAnchor="middle" className="donut-total">{formatMoney(total, currency)}</text>
      </svg>
      <div className="legend">
        {rows.map((row, index) => (
          <span key={row.name}><i style={{ background: palette[index % palette.length] }} /> {row.name} <b>{formatMoney(row.total, currency)}</b></span>
        ))}
      </div>
    </div>
  );
}

function AccountBars({ rows, total, currency = "IDR" }) {
  return (
    <div className="bars">
      {rows.map((row, index) => {
        const percent = total ? Math.round((row.total / total) * 100) : 0;
        return (
          <div className="bar-item" key={row.name}>
            <div><strong>{row.name}</strong><span>{formatMoney(row.total, currency)}</span></div>
            <div className="track"><span style={{ width: `${percent}%`, background: palette[index % palette.length] }} /></div>
            <small>{percent}% of selected month</small>
          </div>
        );
      })}
    </div>
  );
}

function SubscriptionGrid({ rows, currency = "IDR" }) {
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

function TransactionTable({ rows, currency = "IDR" }) {
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

async function requestGoogleAccessToken() {
  await loadGoogleIdentityServices();

  return new Promise((resolve, reject) => {
    const client = window.google.accounts.oauth2.initTokenClient({
      client_id: GOOGLE_CLIENT_ID,
      scope: GOOGLE_SCOPES,
      prompt: "consent",
      callback: (response) => {
        if (response.error) {
          reject(new Error(response.error_description || response.error));
          return;
        }
        const grantedScopes = response.scope ?? "";
        if (!grantedScopes.includes("https://www.googleapis.com/auth/gmail.readonly")) {
          reject(new Error("Google did not grant Gmail read access. Add the Gmail readonly scope in Google Auth Platform, then reconnect."));
          return;
        }
        resolve(response.access_token);
      },
      error_callback: (error) => reject(new Error(error.message || "Google OAuth popup was closed")),
    });
    client.requestAccessToken();
  });
}

async function loadGoogleIdentityServices() {
  if (window.google?.accounts?.oauth2) return;

  await new Promise((resolve, reject) => {
    const existing = document.querySelector("script[data-google-identity]");
    if (existing) {
      existing.addEventListener("load", resolve, { once: true });
      existing.addEventListener("error", reject, { once: true });
      return;
    }

    const script = document.createElement("script");
    script.src = "https://accounts.google.com/gsi/client";
    script.async = true;
    script.defer = true;
    script.dataset.googleIdentity = "true";
    script.onload = resolve;
    script.onerror = () => reject(new Error("Could not load Google Identity Services"));
    document.head.append(script);
  });
}

async function fetchGoogleProfile(token) {
  const response = await fetch("https://www.googleapis.com/oauth2/v3/userinfo", {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!response.ok) throw new Error("Could not read Google profile");
  return response.json();
}

function formatSignal(confidence) {
  if (confidence === "high") return "Account match";
  if (confidence === "low") return "Sender fallback";
  return "Institution match";
}

function sum(rows) {
  return rows.reduce((total, row) => total + row.amount, 0);
}

function totalsBy(rows, key) {
  const map = new Map();
  rows.forEach((row) => map.set(row[key], (map.get(row[key]) ?? 0) + row.amount));
  return [...map].map(([name, total]) => ({ name, total })).sort((a, b) => b.total - a.total);
}

function shiftMonth(month, amount) {
  const [year, monthNumber] = month.split("-").map(Number);
  const date = new Date(year, monthNumber - 1 + amount, 1);
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}`;
}

function latestSubscriptions(rows) {
  const map = new Map();
  rows.filter((row) => row.recurring).forEach((row) => {
    const existing = map.get(row.merchant);
    if (!existing || row.date > existing.date) map.set(row.merchant, row);
  });
  return [...map.values()].sort((a, b) => (a.nextRenewal ?? "").localeCompare(b.nextRenewal ?? ""));
}

function formatDelta(delta) {
  if (!Number.isFinite(delta)) return "No change";
  if (Math.abs(delta) < 0.1) return "Flat";
  return `${delta > 0 ? "+" : ""}${delta.toFixed(1)}%`;
}

function currentMonth() {
  return dateToInputValue(new Date()).slice(0, 7);
}

function recentMonths(count) {
  const current = currentMonth();
  return Array.from({ length: count }, (_, index) => shiftMonth(current, index - count + 1));
}

function apiFetch(path, options) {
  return fetch(`${API_BASE_URL}${path}`, options).then(async (response) => {
    if (!response.ok) {
      const data = await response.json().catch(() => ({}));
      throw new Error(data.error ?? `API request failed: ${response.status}`);
    }
    return response;
  });
}

function formatAiName(config) {
  if (Array.isArray(config.aiProviders) && config.aiProviders.length > 1) {
    return config.aiProviders.map((provider) => provider[0].toUpperCase() + provider.slice(1)).join(" → ");
  }
  if (config.aiProvider === "gemini") return `Gemini ${config.aiModel}`;
  if (config.aiProvider === "ollama") return `Ollama ${config.aiModel}`;
  if (config.aiProvider === "groq") return `Groq ${config.aiModel}`;
  return "AI";
}

function dateToInputValue(date) {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

createRoot(document.getElementById("root")).render(<App />);
