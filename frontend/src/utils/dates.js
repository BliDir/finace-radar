export function currentMonth() {
  return dateToInputValue(new Date()).slice(0, 7);
}

export function recentMonths(count) {
  const current = currentMonth();
  return Array.from({ length: count }, (_, index) => shiftMonth(current, index - count + 1));
}

export function shiftMonth(month, amount) {
  const [year, monthNumber] = month.split("-").map(Number);
  const date = new Date(year, monthNumber - 1 + amount, 1);
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}`;
}

function dateToInputValue(date) {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}
