export const GOOGLE_CLIENT_ID = import.meta.env.VITE_GOOGLE_CLIENT_ID;

export const GOOGLE_SCOPES = [
  "openid",
  "email",
  "profile",
  "https://www.googleapis.com/auth/gmail.readonly",
].join(" ");

export const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? "";

export const chartPalette = ["#0f9f94", "#58d68d", "#087c78", "#a8d4d2", "#2f7f89", "#7fa2a6"];
