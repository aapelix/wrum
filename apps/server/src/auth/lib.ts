import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";
import { emailOTP } from "better-auth/plugins";
import { Resend } from "resend";
import { db } from "../db";
import { anonymous } from "better-auth/plugins";
import * as schema from "../db/schema";
import { dash } from "@better-auth/infra";

const resend = new Resend(process.env.RESEND_API_KEY);

export const auth = betterAuth({
  appName: "wrum",
  database: drizzleAdapter(db, {
    provider: "pg",
    schema,
  }),
  trustedOrigins: [process.env.CLIENT_URL!],
  plugins: [
    dash(),
    anonymous(),
    emailOTP({
      storeOTP: "hashed",
      async sendVerificationOTP({ email, otp, type }) {
        if (type === "sign-in") {
          void resend.emails.send({
            from: "wrum <wrum@auth.aapelix.dev>",
            to: email,
            subject: "Wrum Sign-In OTP",
            html: `<p>Your OTP for signing in to Wrum is: <strong>${otp}</strong></p><p>This OTP will expire in 5 minutes.</p>`,
          });
        }
      },
    }),
  ],
  // only for prod
  advanced:
    process.env.NODE_ENV === "production"
      ? {
          ipAddress: {
            ipAddressHeaders: ["cf-connecting-ip", "x-forwarded-for"],
          },
          defaultCookieAttributes: {
            sameSite: "none",
            secure: true,
          },
        }
      : undefined,
});
