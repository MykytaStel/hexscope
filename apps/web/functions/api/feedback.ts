// POST /api/feedback — see server/feedback.ts.
import { feedback, type Context } from "../../server/handlers";

export const onRequest = (context: Context) => feedback(context);
