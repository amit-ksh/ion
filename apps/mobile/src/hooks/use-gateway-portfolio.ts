import {
  portfolioReadGrant,
  portfolioDirectoryGrants,
  type PortfolioOwner,
  PortfolioQuery,
  PortfolioResult,
} from "@waku/client";
import * as Crypto from "expo-crypto";
import { useFocusEffect } from "expo-router";
import { useCallback, useEffect, useRef, useState } from "react";
import { AppState } from "react-native";
import {
  gatewayPortfolioOwners,
  type GatewayPairing,
} from "@/lib/gateway-client";
import { loadGatewayPortfolio } from "@/lib/gateway-portfolio";
import { readGatewayPairing } from "@/lib/gateway-storage";

type Projects = Extract<PortfolioResult, { kind: "projects" }>["page"];
const emptyProjects = (): Projects => ({
  projects: [],
  nextCursor: null,
  total: 0,
});
function identity(pairing: GatewayPairing | null) {
  return pairing
    ? JSON.stringify([
        pairing.baseUrl,
        pairing.id,
        pairing.token,
        pairing.expiresAt,
        [...pairing.grants].sort(),
      ])
    : "";
}
export function useGatewayPortfolio(
  query: PortfolioQuery,
  options: { ownerId?: string; loadProjects?: boolean } = {},
) {
  const [pairing, setPairing] = useState<GatewayPairing | null>(null);
  const [owners, setOwners] = useState<PortfolioOwner[]>([]);
  const [ownerId, setOwnerId] = useState(options.ownerId ?? "");
  const [projects, setProjects] = useState<Projects>(emptyProjects);
  const [result, setResult] = useState<PortfolioResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const visible = useRef(false),
    generation = useRef(0),
    querying = useRef(false),
    rerun = useRef(false);
  const pairingRef = useRef<GatewayPairing | null>(null),
    ownerRef = useRef(options.ownerId ?? ""),
    projectOwner = useRef("");
  const projectsRef = useRef(projects);
  projectsRef.current = projects;
  const queryRef = useRef(query);
  queryRef.current = query;
  const resultScope = useRef("");
  const key = JSON.stringify(query);
  const scope = () => `${ownerRef.current}:${JSON.stringify(queryRef.current)}`;
  const foreground = () =>
    visible.current && AppState.currentState === "active";
  const clear = () => {
    setResult(null);
    resultScope.current = "";
  };

  async function refresh() {
    if (!foreground()) return;
    if (querying.current) {
      rerun.current = true;
      return;
    }
    querying.current = true;
    rerun.current = false;
    setLoading(true);
    setMessage(null);
    const ticket = generation.current;
    const current = () => foreground() && ticket === generation.current;
    try {
      const saved = await readGatewayPairing();
      if (!current()) return;
      if (identity(saved) !== identity(pairingRef.current)) {
        pairingRef.current = saved;
        setPairing(saved);
        setOwners([]);
        ownerRef.current = options.ownerId ?? "";
        setOwnerId(options.ownerId ?? "");
        projectOwner.current = "";
        projectsRef.current = emptyProjects();
        setProjects(projectsRef.current);
        clear();
      }
      const credential = pairingRef.current;
      const grant = portfolioReadGrant(queryRef.current);
      if (
        !credential ||
        credential.expiresAt <= Date.now() ||
        (grant
          ? !credential.grants.includes(grant)
          : !credential.grants.some((g) =>
              portfolioDirectoryGrants.includes(g),
            ))
      ) {
        clear();
        setMessage(
          `Pair this phone with ${grant === "inboxRead" ? "Read owner Inbox history" : grant === "usageRead" ? "Read owner usage" : grant === "evidenceRead" ? "Read workflow documents" : "Read workflows"} in the gateway owner console.`,
        );
        return;
      }
      const directory = await gatewayPortfolioOwners(credential);
      if (!current()) return;
      setOwners(directory);
      const selected =
        directory.find((owner) => owner.publisherId === ownerRef.current) ??
        (options.ownerId !== undefined
          ? undefined
          : (directory.find(
              (owner) =>
                owner.publishedAt <= Date.now() &&
                Date.now() - owner.publishedAt <= 60_000,
            ) ?? directory[0]));
      if (!selected) {
        clear();
        setMessage(
          "No registered owner has published history. Start the owner daemon and refresh.",
        );
        return;
      }
      ownerRef.current = selected.publisherId;
      setOwnerId(selected.publisherId);
      if (projectOwner.current !== selected.publisherId) {
        projectOwner.current = selected.publisherId;
        projectsRef.current = emptyProjects();
        setProjects(projectsRef.current);
        clear();
      }
      if (
        selected.publishedAt > Date.now() ||
        Date.now() - selected.publishedAt > 60_000
      ) {
        clear();
        setMessage("Owner is offline. Start the owner daemon, then refresh.");
        return;
      }
      const requested = queryRef.current,
        requestedScope = scope();
      const stillCurrent = () =>
        current() &&
        pairingRef.current === credential &&
        scope() === requestedScope;
      // Independent reads: a project-directory failure must not hide a valid history page.
      const reads = await Promise.allSettled([
        loadGatewayPortfolio(
          credential,
          selected.publisherId,
          requested,
          Crypto.randomUUID(),
          stillCurrent,
        ),
        options.loadProjects === false || projectsRef.current.projects.length
          ? Promise.resolve(null)
          : loadGatewayPortfolio(
              credential,
              selected.publisherId,
              { kind: "projects", query: { after: null, limit: 32 } },
              Crypto.randomUUID(),
              stillCurrent,
            ),
      ]);
      if (!stillCurrent()) return;
      const [history, catalog] = reads;
      if (
        catalog.status === "fulfilled" &&
        catalog.value?.kind === "projects"
      ) {
        projectsRef.current = catalog.value.page;
        setProjects(catalog.value.page);
      }
      if (history.status === "rejected") throw history.reason;
      resultScope.current = requestedScope;
      setResult(history.value);
      if (catalog.status === "rejected")
        setMessage(
          "History loaded. Project names are unavailable; refresh to retry.",
        );
    } catch (cause) {
      if (current()) {
        clear();
        setMessage(
          cause instanceof Error
            ? cause.message
            : "Owner history unavailable. Refresh to retry.",
        );
      }
    } finally {
      querying.current = false;
      if (foreground()) {
        setLoading(false);
        if (rerun.current || ticket !== generation.current)
          void refreshRef.current();
      }
    }
  }
  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;
  useEffect(() => {
    if (options.ownerId !== undefined && ownerRef.current !== options.ownerId) {
      ownerRef.current = options.ownerId;
      setOwnerId(options.ownerId);
      projectOwner.current = "";
      projectsRef.current = emptyProjects();
      setProjects(projectsRef.current);
    }
    generation.current += 1;
    clear();
    void refreshRef.current();
  }, [key, options.ownerId]);
  useFocusEffect(
    useCallback(() => {
      visible.current = true;
      generation.current += 1;
      clear();
      void refreshRef.current();
      const subscription = AppState.addEventListener("change", (state) => {
        generation.current += 1;
        clear();
        if (state === "active") void refreshRef.current();
      });
      return () => {
        visible.current = false;
        generation.current += 1;
        subscription.remove();
      };
    }, []),
  );

  function selectOwner(value: string) {
    if (value === ownerRef.current) return;
    ownerRef.current = value;
    setOwnerId(value);
    generation.current += 1;
    projectOwner.current = "";
    projectsRef.current = emptyProjects();
    setProjects(projectsRef.current);
    clear();
    void refreshRef.current();
  }
  async function moreProjects() {
    const credential = pairingRef.current,
      after = projectsRef.current.nextCursor,
      selected = ownerRef.current;
    if (!foreground() || querying.current || !credential || !after) return;
    querying.current = true;
    setLoading(true);
    setMessage(null);
    const ticket = generation.current;
    const current = () =>
      foreground() &&
      ticket === generation.current &&
      pairingRef.current === credential &&
      ownerRef.current === selected;
    try {
      const page = await loadGatewayPortfolio(
        credential,
        selected,
        { kind: "projects", query: { after, limit: 32 } },
        Crypto.randomUUID(),
        current,
      );
      if (!current() || page.kind !== "projects") return;
      const unique = new Map(
        projectsRef.current.projects.map((project) => [project.id, project]),
      );
      page.page.projects.forEach((project) => unique.set(project.id, project));
      projectsRef.current = { ...page.page, projects: [...unique.values()] };
      setProjects(projectsRef.current);
    } catch (cause) {
      if (current())
        setMessage(
          cause instanceof Error
            ? cause.message
            : "Project directory unavailable. Try again.",
        );
    } finally {
      querying.current = false;
      if (foreground()) {
        setLoading(false);
        if (rerun.current || ticket !== generation.current)
          void refreshRef.current();
      }
    }
  }
  return {
    pairing,
    owners,
    ownerId,
    selectOwner,
    projects,
    moreProjects,
    result:
      (options.ownerId === undefined || options.ownerId === ownerId) &&
      resultScope.current === `${ownerId}:${key}`
        ? result
        : null,
    loading,
    message,
    refresh,
  };
}
