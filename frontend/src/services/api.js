import { API_BASE_URL } from "../config";

export function apiFetch(path, options) {
  return fetch(`${API_BASE_URL}${path}`, options).then(async (response) => {
    if (!response.ok) {
      const data = await response.json().catch(() => ({}));
      throw new Error(data.error ?? `API request failed: ${response.status}`);
    }
    return response;
  });
}
