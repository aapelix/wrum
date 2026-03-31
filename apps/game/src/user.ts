import type { User } from "better-auth";
import { authClient } from "./auth/client";

let user: User | null = null;

export async function getUser() {
  if (user) return user;

  const { data, error } = await authClient.getSession();
  if (error) {
    go("error", error.message);
    return;
  }

  if (data) {
    user = data.user;
    return data.user;
  }

  go("login");
}

export async function logout() {
  const { error } = await authClient.signOut();
  user = null;

  return error;
}
