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
  PieChart,
  Search,
  WalletCards,
} from "lucide-react";

import { AccountBars } from "./components/AccountBars";
import { DonutChart, TrendChart } from "./components/Charts";
import { InboxList } from "./components/InboxList";
import { LoginPage } from "./components/LoginPage";
import { Metric } from "./components/Metric";
import { Panel } from "./components/Panel";
import { ProfileSettings } from "./components/ProfileSettings";
import { SubscriptionGrid } from "./components/SubscriptionGrid";
import { TransactionTable } from "./components/TransactionTable";
import { UserMenu } from "./components/UserMenu";
import { GOOGLE_CLIENT_ID } from "./config";
import logoUrl from "./assets/finance-radar-logo-cropped.png";
import { apiFetch } from "./services/api";
import { fetchGoogleProfile, requestGoogleAccessToken } from "./services/googleAuth";
import { loadProfile, saveProfile } from "./services/profileStorage";
import { currentMonth, recentMonths, shiftMonth } from "./utils/dates";
import { dominantCurrency, latestSubscriptions, sum, totalsBy } from "./utils/dashboard";
import { formatAiName, formatDelta, formatMoney } from "./utils/formatters";
import { getTranslations } from "./utils/i18n";

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
  const [profile, setProfile] = useState(() => loadProfile());
  const [googleUser, setGoogleUser] = useState({ name: "", picture: "" });
  const [isUserMenuOpen, setIsUserMenuOpen] = useState(false);
  const [toast, setToast] = useState(null);

  const t = getTranslations(profile.language);
  const isAuthenticated = Boolean(accessToken && signedInEmail);
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

  const displayCurrency = profile.currency || dominantCurrency(transactions);
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
    if (isAuthenticated) loadDashboard(selectedMonth);
  }, [selectedMonth, isAuthenticated]);

  useEffect(() => {
    if (isAuthenticated) loadSpendingTrend();
  }, [isAuthenticated]);

  useEffect(() => {
    if (!isAuthenticated || !scanState) return undefined;
    const message = formatToastMessage(scanState);
    setToast({ message, type: getToastType(scanState) });
    const timer = window.setTimeout(() => setToast(null), 5000);
    return () => window.clearTimeout(timer);
  }, [scanState, isAuthenticated]);

  async function loadDashboard(month) {
    setIsLoading(true);
    try {
      const response = await apiFetch(`/api/dashboard?month=${encodeURIComponent(month)}`);
      const data = await response.json();
      applyDashboardData(data);
      const count = (data.transactions ?? []).length;
      setScanState(count > 0 ? t.showingSavedTransactions(month, count) : t.noSavedData(month));
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
    setScanState(t.requestingGmailAccess);
    try {
      const token = await requestGoogleAccessToken();
      const googleProfile = await fetchGoogleProfile(token).catch(() => ({}));
      const email = googleProfile.email ?? "";
      setAccessToken(token);
      setSignedInEmail(email);
      setGoogleUser({ name: googleProfile.name ?? "", picture: googleProfile.picture ?? "" });
      const savedProfile = loadProfile();
      setProfile(savedProfile);
      setScanState(getTranslations(savedProfile.language).gmailConnected);
    } catch (error) {
      setScanState(error.message || t.gmailConnectionFailed);
    } finally {
      setIsLoading(false);
    }
  }

  async function analyzeInbox(month = selectedMonth) {
    if (!accessToken) {
      setScanState(t.connectGmailFirst);
      return;
    }
    setIsAnalyzing(true);
    setScanState(t.analyzingInbox(month, aiName));
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
      setScanState(t.analyzedSaved(month, count));
    } catch (error) {
      setScanState(error.message || t.inboxAnalysisFailed);
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
    setGoogleUser({ name: "", picture: "" });
    setIsUserMenuOpen(false);
    setProfile(loadProfile());
    setInboxMessages([]);
    setEmailAnalyses(new Map());
    setTransactions([]);
    setTrendByMonth(new Map());
    setQuery("");
    setScanState(GOOGLE_CLIENT_ID ? t.dashboardLoginMessage : t.serviceNotice);
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

  function updateProfile(nextProfile) {
    setProfile(nextProfile);
    saveProfile(nextProfile);
  }

  function analyzeFromMenu() {
    setIsUserMenuOpen(false);
    analyzeInbox();
  }

  function logoutFromMenu() {
    setIsUserMenuOpen(false);
    logout();
  }

  if (!isAuthenticated) {
    return <LoginPage isLoading={isLoading} message={scanState} onLogin={connectGmail} t={t} />;
  }

  return (
    <div className="app">
      <aside className="rail">
        <div className="brand">
          <img className="logo" src={logoUrl} alt="Finance Radar" />
        </div>

        <nav>
          <a href="#overview" className="active"><PieChart size={18} /> {t.overview}</a>
          <a href="#accounts"><WalletCards size={18} /> {t.account}</a>
          <a href="#subscriptions"><Bell size={18} /> {t.subscriptions}</a>
          <a href="#transactions"><Inbox size={18} /> {t.inboxLedger}</a>
        </nav>
      </aside>

      <main>
        <header className="topbar">
          <div className="topbar-left" aria-hidden="true" />
          <div className="topbar-user">
            <ProfileSettings
              profile={profile}
              onChange={updateProfile}
              t={t}
              className="topbar-profile"
              compact
            />
            <UserMenu
              user={googleUser}
              email={signedInEmail}
              isOpen={isUserMenuOpen}
              isAnalyzing={isAnalyzing}
              isLoading={isLoading}
              onToggle={() => setIsUserMenuOpen((open) => !open)}
              onAnalyze={analyzeFromMenu}
              onLogout={logoutFromMenu}
              t={t}
            />
          </div>
        </header>
        {toast && (
          <div className={`toast toast-${toast.type}`} role="status" aria-live="polite">
            <span>{toast.message}</span>
            <button type="button" onClick={() => setToast(null)} aria-label="Close notification">
              &times;
            </button>
          </div>
        )}

        <header className="hero">
          <div>
            <span className="kicker">{t.liveGmailConnection}</span>
            <h1>{signedInEmail ? t.heroTitle : t.connectGmail}</h1>
            <p>{signedInEmail ? t.trackFinance : t.authorizeGmail}</p>
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
                aria-label={t.emailMonth}
              />
            </label>
            <button disabled={selectedIndex === months.length - 1 || isLoading} onClick={() => changeMonth(months[selectedIndex + 1])} aria-label="Next month">
              <ChevronRight size={18} />
            </button>
          </div>
        </header>

        <section id="overview" className="metrics">
          <Metric title={t.monthlySpending} value={formatMoney(sum(monthly.filter((item) => item.direction === "spending" || item.direction === "fee")), displayCurrency)} note={t.totalFlowVsPreviousMonth(formatDelta(delta, t))} icon={<ArrowDownRight size={20} />} />
          <Metric title={t.monthlyIncome} value={formatMoney(sum(monthly.filter((item) => item.direction === "income" || item.direction === "refund")), displayCurrency)} note={t.incomingPayments(monthly.filter((item) => item.direction === "income").length)} icon={<Landmark size={20} />} />
          <Metric title={t.subscriptionRunRate} value={formatMoney(sum(monthly.filter((item) => item.recurring)), displayCurrency)} note={t.recurringCharges(monthly.filter((item) => item.recurring).length)} icon={<Bell size={20} />} />
          <Metric title={t.topAccount} value={accounts[0]?.name ?? t.noData} note={accounts[0] ? formatMoney(accounts[0].total, displayCurrency) : t.readInbox} icon={<CreditCard size={20} />} />
        </section>

        <section className="layout-grid">
          <Panel className="span-8" title={t.spendingTrend} subtitle={t.spendingTrendSubtitle}>
            <TrendChart months={months} trendByMonth={trendByMonth} selectedMonth={selectedMonth} currency={displayCurrency} t={t} />
          </Panel>
          <Panel id="accounts" className="span-4" title={t.paymentAccounts} subtitle={t.paymentAccountsSubtitle}>
            <AccountBars rows={accounts} total={total} currency={displayCurrency} />
          </Panel>
          <Panel className="span-4" title={t.categoryMix} subtitle={t.categoryMixSubtitle}>
            <DonutChart rows={categories} total={total} currency={displayCurrency} t={t} />
          </Panel>
          <Panel id="subscriptions" className="span-8" title={t.subscriptions} subtitle={t.subscriptionsSubtitle}>
            <SubscriptionGrid rows={subscriptions} currency={displayCurrency} t={t} />
          </Panel>
          <Panel
            className="span-12"
            title={t.readEmails}
            subtitle={
              signedInEmail
                ? inboxFinanceOnly
                  ? t.financeMessagesOf(filteredInbox.length, inboxMessages.length, selectedMonth)
                  : t.gmailReadMessages(inboxMessages.length, selectedMonth, signedInEmail)
                : t.unreadInbox
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
                  {t.allEmails}
                </button>
                <button
                  type="button"
                  className={inboxFinanceOnly ? "active" : ""}
                  onClick={() => setInboxFinanceOnly(true)}
                  aria-pressed={inboxFinanceOnly}
                >
                  {t.financeEmails}
                </button>
              </div>
            </div>
            <InboxList rows={filteredInbox} analyses={emailAnalyses} financeOnly={inboxFinanceOnly} t={t} />
          </Panel>
          <Panel id="transactions" className="span-12" title={t.accountLedger} subtitle={t.accountLedgerSubtitle}>
            <div className="toolbar">
              <label className="search-box">
                <Search size={18} />
                <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t.searchPlaceholder} />
              </label>
            </div>
            <TransactionTable rows={filtered} currency={displayCurrency} t={t} />
          </Panel>
        </section>
      </main>
    </div>
  );
}

function getToastType(message) {
  return /error|failed|could not|does not exist/i.test(message) ? "error" : "status";
}

function formatToastMessage(message) {
  if (/relation "emails" does not exist/i.test(message)) {
    return "Database is not ready. Please create the email tables, then refresh the dashboard.";
  }
  return message;
}
