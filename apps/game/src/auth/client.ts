import { createAuthClient } from "better-auth/client";
import { anonymousClient, emailOTPClient } from "better-auth/client/plugins";

const apiUrl = import.meta.env.VITE_API_URL;

export const authClient = createAuthClient({
  baseURL: apiUrl,
  plugins: [emailOTPClient(), anonymousClient()],
});
