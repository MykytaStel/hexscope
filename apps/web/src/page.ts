// What every page but the app itself runs: the theme button in its top bar.
import { themeButton } from "./theme";

document.querySelector(".topbar .actions")?.prepend(themeButton());
