use regex::Regex;

use crate::models::EmailMessage;

pub(super) fn for_email(message: &EmailMessage) -> String {
    format!(
        r#"Analyze this single email for personal finance transaction data.

Return JSON only with this shape:
{{"analyses":[{{"id":"{id}","isFinance":true|false,"direction":"spending|income|transfer|refund|fee|non_finance","amount":number|null,"currency":"IDR|JPY|USD|...|null","date":"YYYY-MM-DD|null","from":"payer/source|null","to":"payee/recipient|null","account":"bank/card/wallet/account identifier|null","accountType":"bank_account|credit_card|debit_card|wallet|unknown|null","merchant":"merchant or institution|null","category":"category|null","confidence":"high|medium|low"}}]}}

Rules:
- Return exactly one analysis for the input id. Do not use facts from other emails.
- A completed money movement is finance even if it is from a no-reply address, has emoji, or is formatted like a notification.
- Treat successful transfers, card charges, debit/credit alerts, receipts, invoices, paid bills, refunds, and bank journals as finance.
- Indonesian card alerts such as "Notifikasi Transaksi Kartu MASTERCARD... di merchant Netflix.com" from BNI or other banks are completed spending.
- Extract parties from labels like source of fund, source account, beneficiary, recipient, merchant, payee, payer, from, to, account, card, rekening, kartu, penerima, merchant/ATM.
- Extract amount and currency from labels like amount, total, charged, paid, debit, credit, transfer amount, transaction amount, sejumlah, nilai, nominal, jumlah, tagihan.
- Recognize Indonesian rupiah as IDR from IDR, Rp, Rp., rupiah, rb/ribu, jt/juta, and formats such as Rp12.500, IDR 12,500, 12.500 rupiah, 600 ribu, 1,5 juta.
- Recognize Japanese yen as JPY from JPY, ¥, 円, yen, and formats such as ¥1,000, JPY 1000, 1,000円, 1000 yen.
- Recognize American dollars as USD from USD, US$, $, dollar, dollars, and formats such as $12.34, US$ 12.34, USD 12.34, 12.34 dollars.
- For JPY, KRW, IDR, and VND, separators are usually thousands separators and the currency has no cents. USD commonly has cents.
- Exclude marketing, referral rewards, cashback offers, newsletters, promotions, ads, job alerts, travel inspiration, crypto promos, news articles, trial/payment-method reminders, and generic service notices when there is no completed transaction amount.
- Job alerts (Jobstreet, LinkedIn jobs, "lowongan", "kandidat kuat"), travel marketing (Tripadvisor inspiration), crypto promos (Indodax giveaways), and "add payment details before trial ends" are never finance.
- Credit card e-billing statements that only show a statement balance or due date without a new charge event are not finance.
- Credit card service notices such as temporary limit increases ("Kenaikan Limit Sementara", "Informasi Permohonan") are not finance.
- Credit card installment promos such as "Cicilan BCA 0%" or "Transaksi Jadi Ringan" are marketing, not completed spending.
- Promo amounts like "win Rp600 ribu" or headline figures in news are not transaction amounts.
- If no actual transaction exists, set isFinance false, direction non_finance, amount null.

Email:
id: {id}
from: {from}
subject: {subject}
date: {date}
snippet: {snippet}
body:
{body}
"#,
        id = message.id,
        from = message.from,
        subject = message.subject,
        date = message.date,
        snippet = message.snippet,
        body = compact_email_for_analysis(&message.body)
    )
}

fn compact_email_for_analysis(body: &str) -> String {
    if body.len() <= 5000 {
        return body.to_string();
    }
    let pattern = Regex::new(
        r"(?i)amount|total|charged|paid|payment|debit|credit|transfer|transaction|status|successful|merchant|beneficiary|recipient|source|account|card|currency|date|reference|sejumlah|nominal|jumlah|nilai|tagihan|transaksi|pembayaran|rekening|kartu|penerima|tanggal",
    )
    .unwrap();
    let lines = body.lines().collect::<Vec<_>>();
    let mut selected = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if pattern.is_match(line) {
            if index > 0 {
                selected.push(lines[index - 1]);
            }
            selected.push(line);
            if index + 1 < lines.len() {
                selected.push(lines[index + 1]);
            }
        }
    }
    let compact = clean_text(&selected.join("\n"));
    if compact.len() > 500 {
        compact.chars().take(5000).collect()
    } else {
        body.chars().take(5000).collect()
    }
}

fn clean_text(value: &str) -> String {
    Regex::new(r"[ \t]+")
        .unwrap()
        .replace_all(value, " ")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
