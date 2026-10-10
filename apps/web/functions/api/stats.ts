// GET /api/stats — the totals the /stats page draws, with the STATS_KEY secret.
import { stats, type Context } from "../../server/handlers";

export const onRequestGet = (context: Context) => stats(context);
