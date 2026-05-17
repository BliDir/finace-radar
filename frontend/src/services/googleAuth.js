import { GOOGLE_CLIENT_ID, GOOGLE_SCOPES } from "../config";

export async function requestGoogleAccessToken() {
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
          reject(
            new Error(
              "Google did not grant Gmail read access. Add the Gmail readonly scope in Google Auth Platform, then reconnect.",
            ),
          );
          return;
        }
        resolve(response.access_token);
      },
      error_callback: (error) => reject(new Error(error.message || "Google OAuth popup was closed")),
    });
    client.requestAccessToken();
  });
}

export async function fetchGoogleProfile(token) {
  const response = await fetch("https://www.googleapis.com/oauth2/v3/userinfo", {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!response.ok) throw new Error("Could not read Google profile");
  return response.json();
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
