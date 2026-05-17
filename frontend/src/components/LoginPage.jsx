import { ShieldCheck } from "lucide-react";

import { GOOGLE_CLIENT_ID } from "../config";
import logoUrl from "../assets/finance-radar-logo-cropped.png";

export function LoginPage({ isLoading, message, onLogin, t }) {
  return (
    <main className="login-page">
      <section className="login-panel" aria-labelledby="login-title">
        <img className="login-logo" src={logoUrl} alt="Finance Radar" />
        <span className="kicker">{t.loginEyebrow}</span>
        <h1 id="login-title">{t.loginTitle}</h1>
        <p>
          {t.loginCopy}
        </p>
        <button type="button" onClick={onLogin} disabled={isLoading || !GOOGLE_CLIENT_ID}>
          <ShieldCheck size={18} /> {isLoading ? t.connecting : t.signInWithGoogle}
        </button>
        <small>{message}</small>
      </section>
    </main>
  );
}
