export function retroGameId(value: string): number | undefined {
  const text = value.trim();
  const id = /^\d+$/.test(text)
    ? text
    : /^https:\/\/retroachievements\.org\/game\/(\d+)\/?$/i.exec(text)?.[1];
  const number = Number(id);
  return Number.isSafeInteger(number) && number > 0 && number <= 0xffffffff
    ? number
    : undefined;
}
