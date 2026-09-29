// What every page but the app itself runs: the theme button in its top bar.
import { themeButton } from "./theme";
import { currentLocale, installLocale, languageButton } from "./i18n";

installLocale();
document.querySelector(".topbar .actions")?.prepend(languageButton(), themeButton());

// The shared controls are localized, while the guide copy is still English.
// Keep its language correct for assistive technology and say so when Ukrainian
// is selected instead of presenting untranslated article text as localized.
const guide = document.querySelector<HTMLElement>(".guide-layout .story-text");
if (guide) {
  guide.lang = "en";
  const notice = document.createElement("p");
  notice.className = "guide-note guide-language-note";
  notice.setAttribute("role", "note");
  notice.textContent = "This guide is currently available in English. The page controls are in Ukrainian.";
  notice.hidden = currentLocale() === "en";
  guide.prepend(notice);
  window.addEventListener("hexscope:locale", () => {
    notice.hidden = currentLocale() === "en";
  });
}
