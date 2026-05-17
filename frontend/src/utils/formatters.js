const moneyFormatters = new Map();

export function formatMoney(amount, currency = "IDR") {
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

export function formatDelta(delta) {
  if (!Number.isFinite(delta)) return "No change";
  if (Math.abs(delta) < 0.1) return "Flat";
  return `${delta > 0 ? "+" : ""}${delta.toFixed(1)}%`;
}

export function formatSignal(confidence) {
  if (confidence === "high") return "Account match";
  if (confidence === "low") return "Sender fallback";
  return "Institution match";
}

export function formatAiName(config) {
  if (Array.isArray(config.aiProviders) && config.aiProviders.length > 1) {
    return config.aiProviders.map((provider) => provider[0].toUpperCase() + provider.slice(1)).join(" -> ");
  }
  if (config.aiProvider === "gemini") return `Gemini ${config.aiModel}`;
  if (config.aiProvider === "ollama") return `Ollama ${config.aiModel}`;
  if (config.aiProvider === "groq") return `Groq ${config.aiModel}`;
  return "AI";
}
