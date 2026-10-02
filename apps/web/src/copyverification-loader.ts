// Both the copy worker request and its report view use the same deferred module.
export const loadCopyVerification = () =>
  Promise.race([
    import("./copyverification"),
    new Promise<never>((_, reject) => setTimeout(reject, 1e4)),
  ]);
