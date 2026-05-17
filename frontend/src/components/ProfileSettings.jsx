import { Globe2, WalletCards } from "lucide-react";

export const defaultProfile = {
  currency: "USD",
  language: "en",
};

const currencies = [
  { value: "USD", label: "USD" },
  { value: "IDR", label: "IDR" },
  { value: "JPY", label: "JPY" },
];

const languages = [
  { value: "en", label: "EN" },
  { value: "id", label: "ID" },
];

export function ProfileSettings({ profile, onChange, t, className = "", compact = false }) {
  return (
    <section className={`profile-card ${className}`} aria-labelledby={compact ? undefined : "profile-settings-title"}>
      {!compact && <h2 id="profile-settings-title">{t.profile}</h2>}
      <label>
        <span title={t.dataCurrency}><WalletCards size={15} /></span>
        <select
          aria-label={t.dataCurrency}
          value={profile.currency}
          onChange={(event) => onChange({ ...profile, currency: event.target.value })}
        >
          {currencies.map((currency) => (
            <option key={currency.value} value={currency.value}>
              {currency.label}
            </option>
          ))}
        </select>
      </label>
      <label>
        <span title={t.language}><Globe2 size={15} /></span>
        <select
          aria-label={t.language}
          value={profile.language}
          onChange={(event) => onChange({ ...profile, language: event.target.value })}
        >
          {languages.map((language) => (
            <option key={language.value} value={language.value}>
              {language.label}
            </option>
          ))}
        </select>
      </label>
    </section>
  );
}
