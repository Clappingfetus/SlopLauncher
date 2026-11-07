import type { Source } from "./types";
import steam from "../assets/source-icons/steam.ico";
import epic from "../assets/source-icons/epic.png";
import amazon from "../assets/source-icons/amazon.png";
import gog from "../assets/source-icons/gog.ico";
import itch from "../assets/source-icons/itch.svg";

// Bundled locally so source icons stay available offline.
export const sourceIcons: Partial<Record<Source, string>> = {
  steam,
  epic,
  amazon,
  gog,
  itch,
};
