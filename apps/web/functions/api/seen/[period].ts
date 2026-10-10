// GET /api/seen/day | month | new — see server/count.ts.
import { seen, type Context } from "../../../server/handlers";

export const onRequestGet = (context: Context) => seen(context);
