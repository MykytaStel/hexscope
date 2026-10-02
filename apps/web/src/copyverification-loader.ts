// Both the copy worker request and its report view use the same deferred module.
export function loadCopyVerification() {
  return import("./copyverification");
}
