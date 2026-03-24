import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";
import { emailOTP } from "better-auth/plugins";
import { Resend } from "resend";
import { db } from "../db";
import * as schema from "../db/schema";

const resend = new Resend(process.env.RESEND_API_KEY);

export const auth = betterAuth({
  database: drizzleAdapter(db, {
    provider: "pg",
    schema,
  }),
  trustedOrigins: [process.env.CLIENT_URL!],
  plugins: [
    emailOTP({
      async sendVerificationOTP({ email, otp, type }) {
        if (type === "sign-in") {
          resend.emails.send({
            from: "wrum <wrum@auth.aapelix.dev>",
            to: email,
            subject: "Wrum Sign-In OTP",
            html: `<p>Your OTP for signing in to Wrum is: <strong>${otp}</strong></p><p>This OTP will expire in 5 minutes.</p>`,
          });
        }
      },
    }),
  ],
});
