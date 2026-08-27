import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronDown, ChevronUp, Eye, EyeOff } from "lucide-react";

import { api, errorMessage, type Region } from "@/lib/api";
import { useApp } from "@/lib/app-context";
import { Button, Card, Muted, Notice } from "@/components/ui";

/**
 * Region curation.
 *
 * Reordering is up/down buttons rather than drag: with a handful of kept
 * Regions it is fewer interactions than a drag, and it works from the keyboard.
 */
export function Settings() {
  const { provider } = useApp();
  const qc = useQueryClient();

  const regions = useQuery({
    queryKey: ["regions", provider.id],
    queryFn: () => api.regions(provider.id),
  });

  const invalidate = () => qc.invalidateQueries();

  const setVisible = useMutation({
    mutationFn: ({ code, visible }: { code: string; visible: boolean }) =>
      api.setRegionVisible(provider.id, code, visible),
    onSuccess: invalidate,
  });

  const reorder = useMutation({
    mutationFn: (codes: string[]) => api.setRegionOrder(provider.id, codes),
    onSuccess: invalidate,
  });

  if (regions.isPending) return <Notice title="Loading…" />;
  if (regions.error)
    return <Notice title="Could not read settings">{errorMessage(regions.error)}</Notice>;

  const shown = regions.data.filter((r) => r.visible);
  const hidden = regions.data.filter((r) => !r.visible);

  const move = (code: string, delta: number) => {
    const order = shown.map((r) => r.code);
    const i = order.indexOf(code);
    const j = i + delta;
    if (j < 0 || j >= order.length) return;
    [order[i], order[j]] = [order[j], order[i]];
    reorder.mutate(order);
  };

  return (
    <div className="overflow-y-auto p-6">
      <div className="mx-auto max-w-2xl space-y-8">
        <header>
          <h1 className="text-lg font-semibold">Regions</h1>
          <Muted className="mt-1">
            Hidden Regions disappear from the category rail and from search.
            Nothing is deleted, so switching one back on is instant.
          </Muted>
        </header>

        <section>
          <h2 className="mb-2 text-sm font-semibold">
            Shown{shown.length ? ` · ${shown.length}` : ""}
          </h2>
          {shown.length === 0 ? (
            <Muted>Nothing shown. The rail will be empty.</Muted>
          ) : (
            <Card className="divide-y divide-[var(--border)]">
              {shown.map((r, i) => (
                <Row
                  key={r.code}
                  region={r}
                  onToggle={() =>
                    setVisible.mutate({ code: r.code, visible: false })
                  }
                  onUp={i > 0 ? () => move(r.code, -1) : undefined}
                  onDown={
                    i < shown.length - 1 ? () => move(r.code, 1) : undefined
                  }
                />
              ))}
            </Card>
          )}
        </section>

        {hidden.length > 0 ? (
          <section>
            <h2 className="mb-2 text-sm font-semibold">
              Hidden · {hidden.length}
            </h2>
            <Card className="divide-y divide-[var(--border)] opacity-60">
              {hidden.map((r) => (
                <Row
                  key={r.code}
                  region={r}
                  onToggle={() =>
                    setVisible.mutate({ code: r.code, visible: true })
                  }
                />
              ))}
            </Card>
          </section>
        ) : null}
      </div>
    </div>
  );
}

function Row({
  region,
  onToggle,
  onUp,
  onDown,
}: {
  region: Region;
  onToggle: () => void;
  onUp?: () => void;
  onDown?: () => void;
}) {
  return (
    <div className="flex items-center gap-2 px-3 py-2">
      <span className="min-w-0 flex-1 truncate text-sm">
        {region.label}
        {region.isNew ? (
          <span className="ml-2 rounded bg-[var(--primary)] px-1.5 py-0.5 text-[10px] font-medium text-[var(--primary-foreground)]">
            new
          </span>
        ) : null}
      </span>
      <span className="shrink-0 text-xs tabular-nums text-[var(--muted-foreground)]">
        {region.categoryCount}
      </span>
      {onUp || onDown ? (
        <span className="flex shrink-0">
          <Button size="sm" variant="ghost" disabled={!onUp} onClick={onUp}>
            <ChevronUp className="size-3.5" />
          </Button>
          <Button size="sm" variant="ghost" disabled={!onDown} onClick={onDown}>
            <ChevronDown className="size-3.5" />
          </Button>
        </span>
      ) : null}
      <Button size="sm" variant="ghost" onClick={onToggle}>
        {region.visible ? (
          <Eye className="size-4" />
        ) : (
          <EyeOff className="size-4" />
        )}
      </Button>
    </div>
  );
}
