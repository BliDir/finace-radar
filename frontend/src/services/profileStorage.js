import { defaultProfile } from "../components/ProfileSettings";

const profileCookieName = "finance_radar_profile";
const maxAgeSeconds = 60 * 60 * 24 * 365;

function normalizeProfile(profile) {
  return {
    ...defaultProfile,
    ...profile,
    currency: ["IDR", "JPY", "USD"].includes(profile?.currency) ? profile.currency : defaultProfile.currency,
    language: ["en", "id"].includes(profile?.language) ? profile.language : defaultProfile.language,
  };
}

function readCookie(name) {
  const value = document.cookie
    .split("; ")
    .find((row) => row.startsWith(`${name}=`))
    ?.split("=")[1];
  return value ? decodeURIComponent(value) : "";
}

export function loadProfile() {
  try {
    const saved = readCookie(profileCookieName);
    return saved ? normalizeProfile(JSON.parse(saved)) : defaultProfile;
  } catch {
    return defaultProfile;
  }
}

export function saveProfile(profile) {
  document.cookie = `${profileCookieName}=${encodeURIComponent(JSON.stringify(normalizeProfile(profile)))}; Max-Age=${maxAgeSeconds}; Path=/; SameSite=Lax`;
}
