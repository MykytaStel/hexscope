// What every page but the app itself runs: the theme button in its top bar.
import { GUIDE_UK } from "./guide-i18n";
import { themeButton } from "./theme";
import { currentLocale, installLocale, languageButton, registerLazyTranslations } from "./i18n";

registerLazyTranslations(GUIDE_UK);
installLocale();
document.querySelector(".topbar .actions")?.prepend(languageButton(), themeButton());

// Keep the article's language in step with the shared language selector so
// assistive technology reads every guide in the language currently displayed.
const guide = document.querySelector<HTMLElement>(".story-text");
if (guide) {
  const syncLanguage = () => {
    const locale = currentLocale();
    document.documentElement.lang = locale;
    guide.lang = locale;
  };
  syncLanguage();
  window.addEventListener("hexscope:locale", syncLanguage);
}
