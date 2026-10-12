// What every page but the app itself runs: the theme button in its top bar.
import { GUIDE_UK } from "./guide-i18n";
import { themeButton } from "./theme";
import { currentLocale, installLocale, languageButton, registerLazyTranslations } from "./i18n";
import { countVisit } from "./count";
import { openFeedback } from "./feedback";
import { showWays } from "./support";

registerLazyTranslations(GUIDE_UK);
installLocale();
document.querySelector(".topbar .actions")?.prepend(languageButton(), themeButton());
countVisit();

const ways = document.getElementById("support-ways");
if (ways) showWays(ways);
for (const b of document.querySelectorAll<HTMLButtonElement>("[data-feedback]")) b.addEventListener("click", () => openFeedback({ kind: "idea" }));

// At the end of every article: a way to tell the developer, and to thank them.
const article = document.querySelector<HTMLElement>(".story-text");
if (article && !ways) {
  const foot = document.createElement("p");
  foot.className = "story-foot story-reach";
  const write = document.createElement("button");
  write.className = "link";
  write.textContent = "Something wrong or missing? Write to the developer";
  write.addEventListener("click", () => openFeedback({ kind: "idea" }));
  const support = document.createElement("a");
  // Beside the article: /support, or /uk/support from a Ukrainian page.
  support.href = "support";
  support.textContent = "Support the project";
  foot.append(write, document.createTextNode(" · "), support);
  article.append(foot);
}

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

  const layout = guide.closest<HTMLElement>(".guide-layout");
  const headings = Array.from(guide.querySelectorAll<HTMLElement>(":scope > section > h2"));
  if (layout && headings.length) {
    const toc = document.createElement("nav");
    toc.className = "guide-toc";
    const title = document.createElement("h2");
    title.textContent = "On this page";
    title.id = "guide-toc-title";
    toc.setAttribute("aria-labelledby", title.id);

    const list = document.createElement("ol");
    const links = headings.map((heading, index) => {
      heading.id = `guide-section-${index + 1}`;
      heading.tabIndex = -1;
      const link = document.createElement("a");
      link.href = `#${heading.id}`;
      const item = document.createElement("li");
      item.append(link);
      list.append(item);
      return link;
    });

    const syncContents = () => {
      headings.forEach((heading, index) => {
        const label = heading.cloneNode(true) as HTMLElement;
        label.querySelector(".num")?.remove();
        links[index].textContent = label.textContent?.replace(/\s+/g, " ").trim() ?? "";
      });
    };

    toc.append(title, list);
    layout.append(toc);
    syncContents();
    window.addEventListener("hexscope:locale", syncContents);

    let scrollFrame = 0;
    const syncCurrentSection = () => {
      if (scrollFrame) return;
      scrollFrame = window.requestAnimationFrame(() => {
        scrollFrame = 0;
        let current = 0;
        headings.forEach((heading, index) => {
          if (heading.getBoundingClientRect().top <= 112) current = index;
        });
        links.forEach((link, index) => {
          if (index === current) link.setAttribute("aria-current", "location");
          else link.removeAttribute("aria-current");
        });
      });
    };
    window.addEventListener("scroll", syncCurrentSection, { passive: true });
    window.addEventListener("resize", syncCurrentSection);
    syncCurrentSection();
  }
}
