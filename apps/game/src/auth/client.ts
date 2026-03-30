import { createAuthClient } from "better-auth/client";
import { anonymousClient, emailOTPClient } from "better-auth/client/plugins";

const apiUrl = import.meta.env.VITE_API_URL;
const authPath = `${apiUrl}/api/auth`;

export const authClient = createAuthClient({
  baseURL: authPath,
  plugins: [emailOTPClient(), anonymousClient()],
});
