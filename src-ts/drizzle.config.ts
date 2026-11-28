import type { Config } from "drizzle-kit";

export default {
  schema: "./src-ts/db/schema.ts",
  out: "./src-ts/db/migrations",
  driver: "better-sqlite3",
  dbCredentials: {
    url: process.env.WIREDESK_DB_PATH ?? "./data/wire-desk.db",
  },
  verbose: true,
  strict: true,
} satisfies Config;
