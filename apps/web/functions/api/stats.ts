// GET /api/stats — the public totals the /stats page draws.
import { stats, type Context } from "../../server/handlers";

export const onRequestGet = (context: Context) => stats(context);
