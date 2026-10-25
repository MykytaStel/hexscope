import { LatestFilmQueue } from "./film-queue";
import type { OperationRequest } from "./worker";

export type FilmOperationRequest = Extract<OperationRequest, { type: "filmRoll" | "filmRender" }>;

export function createFilmQueue(
  run: (request: FilmOperationRequest) => Promise<void>,
  discard: (request: FilmOperationRequest) => void,
): LatestFilmQueue<FilmOperationRequest> {
  return new LatestFilmQueue(run, discard);
}
