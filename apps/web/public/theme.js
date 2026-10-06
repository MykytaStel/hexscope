// Applies the chosen theme before the page is painted, so it never flashes
// in the other one: dark by default, or a saved light, dark, or system choice.
// A file of its own because the page allows no inline script.
try {
  var t = localStorage.getItem("hexscope.theme");
  if (t === "light" || t === "dark") document.documentElement.dataset.theme = t;
  else if (t === "auto") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = "dark";
} catch (e) {
  // No storage: the graphite default.
  document.documentElement.dataset.theme = "dark";
}
try {
  var savedLanguage = localStorage.getItem("hexscope.language");
  var ukrainian = savedLanguage === "uk" || (savedLanguage !== "en" && navigator.languages.some((language) => language.toLowerCase().startsWith("uk")));
  document.documentElement.lang = ukrainian ? "uk" : "en";
} catch (e) {
  // English remains the default when storage or browser language is unavailable.
}
