/** Keeps worker-owned WebAssembly values alive only for the operation using them. */
export async function withWorkerResource<T extends { free(): void }, R>(resource: T, use: (resource: T) => Promise<R>): Promise<R> {
  try {
    return await use(resource);
  } finally {
    resource.free();
  }
}
