const config = {
  "*.{cjs,mjs,js,ts,jsx,tsx}": (stagedFiles) => [
    `eslint --fix ${stagedFiles.join(" ")}`,
    `prettier --write ${stagedFiles.join(" ")}`,
  ],
  "*.{css,md,mdx,json}": (stagedFiles) => [
    `prettier --write ${stagedFiles.join(" ")}`,
  ],
};
export default config;
