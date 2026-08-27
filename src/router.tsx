import {
  createRootRoute,
  createRoute,
  createRouter,
  redirect,
} from "@tanstack/react-router";

import { App } from "@/App";
import { Catalogue, SeriesDetail } from "@/components/Catalogue";
import { useApp } from "@/lib/app-context";

const rootRoute = createRootRoute({ component: App });

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  beforeLoad: () => {
    throw redirect({ to: "/live" });
  },
});

function CatalogueRoute({ kind }: { kind: "live" | "movie" | "series" }) {
  const { provider, play } = useApp();
  return <Catalogue providerId={provider.id} kind={kind} onPlay={play} />;
}

const liveRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/live",
  component: () => <CatalogueRoute kind="live" />,
});

const moviesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/movies",
  component: () => <CatalogueRoute kind="movie" />,
});

const seriesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/series",
  component: () => <CatalogueRoute kind="series" />,
});

const seriesDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/series/$seriesId",
  component: function SeriesDetailRoute() {
    const { provider, play } = useApp();
    const { seriesId } = seriesDetailRoute.useParams();
    return (
      <SeriesDetail
        providerId={provider.id}
        seriesId={Number(seriesId)}
        onPlay={play}
      />
    );
  },
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  liveRoute,
  moviesRoute,
  seriesRoute,
  seriesDetailRoute,
]);

export const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
