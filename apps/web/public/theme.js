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
  // A page written in Ukrainian (/uk/) stays so unless English was chosen.
  var written = document.documentElement.lang === "uk";
  // Kept, as the language set below may be another: the page knows what to translate.
  document.documentElement.dataset.written = written ? "uk" : "en";
  var ukrainian = savedLanguage === "uk" || (savedLanguage !== "en" && (written || navigator.languages.some((language) => language.toLowerCase().startsWith("uk"))));
  document.documentElement.lang = ukrainian ? "uk" : "en";
} catch (e) {
  // English remains the default when storage or browser language is unavailable.
}
