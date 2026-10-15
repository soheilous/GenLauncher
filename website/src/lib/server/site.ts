const env = (globalThis as { process?: { env: Record<string, string | undefined> } }).process?.env ?? {};

/** The site's address, for canonical and hreflang links and the sitemap (SITE_URL, or the production address Vercel provides); empty when unknown. */
export const siteOrigin = (): string =>
  (env.SITE_URL ?? (env.VERCEL_PROJECT_PRODUCTION_URL ? `https://${env.VERCEL_PROJECT_PRODUCTION_URL}` : "")).replace(/\/$/, "");
