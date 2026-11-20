import { readable } from "svelte/store";

export const onlineVersion = readable(0, (set) => {
  if (typeof window === "undefined") return;
  let version = 0;
  const onOnline = () => set(++version);
  window.addEventListener("online", onOnline);
  return () => window.removeEventListener("online", onOnline);
});
