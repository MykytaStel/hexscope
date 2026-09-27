// Applies the chosen theme before the page is painted, so it never flashes
// in the other one: light, dark, or — with nothing chosen — the system's.
// A file of its own because the page allows no inline script.
try {
  var t = localStorage.getItem("hexscope.theme");
  if (t === "light" || t === "dark") document.documentElement.dataset.theme = t;
} catch (e) {
  // No storage: the system's theme.
}
