import { Container } from "@cloudflare/containers";

export class AfterSwapContainer extends Container {
  defaultPort = 8787;
  // Keep the engine loop alive well past a judging session; a visit
  // restarts it after sleep (leaderboard rebuilds in ~30s).
  sleepAfter = "2h";
  enableInternet = true; // outbound DFlow quote polling
}

interface Env {
  AFTERSWAP: DurableObjectNamespace<AfterSwapContainer>;
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const container = env.AFTERSWAP.getByName("singleton");
    await container.startAndWaitForPorts();
    // fetch() (not containerFetch) so streaming/SSE pass through.
    return container.fetch(request);
  },
};
