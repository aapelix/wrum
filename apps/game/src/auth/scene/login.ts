import { addButton } from "../../ui/button";
import { addInput } from "../../ui/input";
import { authClient } from "../client";

export function loadLoginScene() {
  const w = width();
  const h = height();

  scene("login", () => {
    let email = "";

    addInput(w / 2, h / 2 - 25, 105, 15, "Email", (s) => {
      email = s;
    });

    addButton(
      w / 2 - 27.5,
      h / 2 + 20,
      50,
      15,
      "Login",
      async () => {
        const { data, error } = await authClient.emailOtp.sendVerificationOtp({
          email,
          type: "sign-in",
        });

        if (error) {
          go("error", error);
          return;
        } else if (data?.success) {
          go("otp", email);
        }
      },
      9,
    );

    addButton(
      w / 2,
      h / 2 + 40,
      105,
      15,
      "Play as Guest",
      async () => {
        if (localStorage.getItem("wrum-anonymous") === "true") {
          go("main");
          return;
        }

        const { data, error } = await authClient.signIn.anonymous();

        if (error) {
          go("error", error);
          return;
        } else if (data !== null) {
          localStorage.setItem("wrum-anonymous", "true");
          go("main", data.user);
        }
      },
      9,
      undefined,
      "secondary",
    );
    addButton(w / 2 + 27.5, h / 2 + 20, 50, 15, "Cancel", () => {}, 9, undefined, "secondary");
  });

  scene("otp", (email) => {
    let otp = "";

    add([
      text("Enter the code sent to you", { size: 9 }),
      pos(w / 2, h / 2 - 50),
      anchor("center"),
    ]);

    addInput(w / 2, h / 2 - 25, 105, 15, "", (s) => {
      otp = s;
    });

    addButton(
      w / 2 - 27.5,
      h / 2 + 20,
      50,
      15,
      "Login",
      async () => {
        const { data, error } = await authClient.signIn.emailOtp({
          email,
          otp,
        });

        if (error) {
          go("error", error);
          return;
        } else if (data !== null) {
          if (localStorage.getItem("wrum-anonymous") === "true") {
            localStorage.removeItem("wrum-anonymous");
          }

          go("main", data.user);
        }
      },
      9,
    );
    addButton(w / 2 + 27.5, h / 2 + 20, 50, 15, "Cancel", () => {}, 9, undefined, "secondary");
  });
}
