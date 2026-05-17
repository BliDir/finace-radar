import { useEffect, useMemo, useState } from "react";
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

import { AccountBars } from "./components/AccountBars";
import { DonutChart, TrendChart } from "./components/Charts";
import { InboxList } from "./components/InboxList";
import { Metric } from "./components/Metric";
import { Panel } from "./components/Panel";
import { SubscriptionGrid } from "./components/SubscriptionGrid";
import { TransactionTable } from "./components/TransactionTable";
import { GOOGLE_CLIENT_ID } from "./config";
import logoUrl from "./assets/finance-radar-logo-cropped.png";
import { apiFetch } from "./services/api";
import { fetchGoogleProfile, requestGoogleAccessToken } from "./services/googleAuth";
import { currentMonth, recentMonths, shiftMonth } from "./utils/dates";
import { dominantCurrency, latestSubscriptions, sum, totalsBy } from "./utils/dashboard";
import { formatAiName, formatDelta, formatMoney } from "./utils/formatters";

export default function App() {
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
          : `${month}: no saved data yet - connect Gmail and click Analyze inbox`,
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
