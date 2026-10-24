import { it, expect, vi } from "vitest";
import { demoGames } from "./demo";
import { EnrichmentQueue } from "./enrichment";
import {
  baselineMetadata,
  needsDiscovery,
  recordMetadataAttempt,
  ManualMetadataScan,
} from "./metadata-scan";
import type { Game } from "./types";
const settle = async () => {
  for (let i = 0; i < 100; i++) await Promise.resolve();
};
it("baselines existing libraries, survives restart, and admits late additions and changed identities", () => {
  const old = baselineMetadata(demoGames()[0]);
  expect(needsDiscovery(old, "steam")).toBe(false);
  expect(needsDiscovery(JSON.parse(JSON.stringify(old)), "pcgw")).toBe(false);
  expect(needsDiscovery({ ...old, title: "Rematched title" }, "steam")).toBe(
    true,
  );
  expect(
    needsDiscovery(
      { ...old, copies: [{ ...old.copies[0], sourceId: "123" }] },
      "steam",
    ),
  ).toBe(true);
  expect(
    needsDiscovery({ ...demoGames()[1], metadataScan: undefined }, "steam"),
  ).toBe(true);
});
it("attempts each provider once even after transient failures, expiry, focus reconciles and restart", async () => {
  let games: Game[] = [{ ...demoGames()[0], metadataScan: undefined }];
  let time = 1000;
  const fetch = vi.fn(async () => ({
    status: "unavailable" as const,
    checkedAt: time,
    nextCheckAt: time + 1,
  }));
  const queue = new EnrichmentQueue({
    getGames: () => games,
    now: () => time,
    enabled: (p) => p === "steam",
    shouldFetch: needsDiscovery,
    started: (g, p) => {
      games = games.map((x) =>
        x.id === g.id ? recordMetadataAttempt(x, p) : x,
      );
    },
    fetch,
    apply: (id, p, v) => {
      games = games.map((g) =>
        g.id === id ? { ...g, enrichment: { ...g.enrichment, [p]: v } } : g,
      );
    },
  });
  queue.reconcile(games);
  await settle();
  expect(fetch).toHaveBeenCalledTimes(1);
  expect(games[0].enrichment?.steam?.status).toBe("unavailable");
  time += 10000000;
  games = JSON.parse(JSON.stringify(games));
  queue.clear();
  queue.reconcile(games);
  await settle();
  expect(fetch).toHaveBeenCalledTimes(1);
  games = [...games, { ...demoGames()[1], metadataScan: undefined }];
  queue.reconcile(games);
  await settle();
  expect(fetch).toHaveBeenCalledTimes(2);
  games = games.map((g, i) => (i === 0 ? { ...g, title: "New identity" } : g));
  queue.reconcile(games);
  await settle();
  expect(fetch).toHaveBeenCalledTimes(3);
  queue.clear();
});
it("manual permissions work when automatic scans are off and do not follow a rematch", async () => {
  let games = [baselineMetadata(demoGames()[0])];
  const manual = new ManualMetadataScan();
  manual.start(games);
  const fetch = vi.fn(async () => ({
    status: "ready" as const,
    checkedAt: 100,
    nextCheckAt: 101,
    data: { summary: "Found" },
  }));
  const queue = new EnrichmentQueue({
    getGames: () => games,
    enabled: (p, g) => p === "steam" && manual.contains(g),
    shouldFetch: (g, p) => manual.allows(g, p),
    started: (g, p) => manual.started(g, p),
    fetch,
    apply: (id, p, v) => {
      games = games.map((g) =>
        g.id === id ? { ...g, enrichment: { ...g.enrichment, [p]: v } } : g,
      );
    },
    onIdle: () => manual.clear(),
  });
  queue.reconcile(games);
  await settle();
  expect(fetch).toHaveBeenCalledTimes(1);
  expect(games[0].enrichment?.steam?.data?.summary).toBe("Found");
  queue.reconcile(games);
  await settle();
  expect(fetch).toHaveBeenCalledTimes(1);
  manual.start(games);
  expect(manual.allows({ ...games[0], title: "Changed" }, "steam")).toBe(false);
  queue.clear();
});
it("removed games never receive a late metadata result", async () => {
  let games = [demoGames()[0]],
    complete!: (v: any) => void;
  const apply = vi.fn();
  const queue = new EnrichmentQueue({
    getGames: () => games,
    enabled: (p) => p === "steam",
    shouldFetch: needsDiscovery,
    started: (g, p) => {
      games = [recordMetadataAttempt(g, p)];
    },
    fetch: () => new Promise((r) => (complete = r)),
    apply,
  });
  queue.reconcile(games);
  games = [];
  complete({ status: "ready", checkedAt: 1, nextCheckAt: 2 });
  await settle();
  expect(apply).not.toHaveBeenCalled();
  queue.clear();
});
it("discovers dependent HLTB data in the same pass without reopening completed providers",async()=>{
 let games=[{...demoGames()[0],enrichment:undefined,metadataScan:undefined} as Game];
 const fetched:string[]=[];
 const queue=new EnrichmentQueue({getGames:()=>games,enabled:p=>p==="pcgw"||p==="hltb",shouldFetch:needsDiscovery,
 started:(g,p)=>{games=games.map(x=>x.id===g.id?recordMetadataAttempt(x,p):x)},
 fetch:async(_g,p)=>{fetched.push(p);return {status:"ready",checkedAt:100,nextCheckAt:1000,data:p==="pcgw"?{hltbId:7}:{extraHours:12}}},
 apply:(id,p,v)=>{games=games.map(g=>g.id===id?{...g,enrichment:{...g.enrichment,[p]:v}}:g)}});
 queue.reconcile(games);await settle();expect(fetched).toEqual(["pcgw","hltb"]);expect(games[0].enrichment?.hltb?.data?.extraHours).toBe(12);
 queue.reconcile(games);await settle();expect(fetched).toHaveLength(2);queue.clear();
});
