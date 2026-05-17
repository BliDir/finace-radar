export function dominantCurrency(rows, fallback = "IDR") {
  const counts = new Map();
  rows.forEach((row) => {
    const code = row.currency;
    if (!code) return;
    counts.set(code, (counts.get(code) ?? 0) + 1);
  });
  if (!counts.size) return fallback;
  return [...counts.entries()].sort((a, b) => b[1] - a[1])[0][0];
}

export function sum(rows) {
  return rows.reduce((total, row) => total + row.amount, 0);
}

export function totalsBy(rows, key) {
  const map = new Map();
  rows.forEach((row) => map.set(row[key], (map.get(row[key]) ?? 0) + row.amount));
  return [...map].map(([name, total]) => ({ name, total })).sort((a, b) => b.total - a.total);
}

export function latestSubscriptions(rows) {
  const map = new Map();
  rows.filter((row) => row.recurring).forEach((row) => {
    const existing = map.get(row.merchant);
    if (!existing || row.date > existing.date) map.set(row.merchant, row);
  });
  return [...map.values()].sort((a, b) => (a.nextRenewal ?? "").localeCompare(b.nextRenewal ?? ""));
}
