import { writable } from "svelte/store";
import type { Game } from "./types";

export const GAME_DRAG_TYPE = "application/x-sloplauncher-game";
// Gesture state is memory-only. Drops must still validate against the live library.
export const draggedGameId = writable<string | null>(null);
let preview: HTMLElement | undefined;
export function endGameDrag() {
  draggedGameId.set(null);
  preview?.remove();
  preview = undefined;
}
export function startGameDrag(event: DragEvent, game: Game) {
  if (!event.dataTransfer) return;
  endGameDrag();
  event.dataTransfer.effectAllowed = "copy";
  event.dataTransfer.setData(GAME_DRAG_TYPE, game.id);
  preview = document.createElement("div");
  preview.className = "collection-drag-preview";
  const image = (event.currentTarget as HTMLElement).querySelector("img");
  if (image) {
    const cover = document.createElement("img");
    cover.src = image.currentSrc || image.src;
    cover.alt = "";
    preview.append(cover);
  }
  const text = document.createElement("span");
  text.textContent = game.title;
  const hint = document.createElement("small");
  hint.textContent = "Add to collection";
  text.append(hint);
  preview.append(text);
  document.body.append(preview);
  event.dataTransfer.setDragImage(preview, 24, 28);
  draggedGameId.set(game.id);
}
