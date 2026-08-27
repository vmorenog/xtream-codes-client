import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { api, errorMessage } from "@/lib/api";
import { Button, Card, Input, Muted, Notice } from "@/components/ui";

/** Shown when mpv is not on the machine (ADR-0003). */
export function MpvMissing({ onRecheck }: { onRecheck: () => void }) {
  return (
    <Notice
      title="mpv is not installed"
      action={
        <Button variant="primary" onClick={onRecheck}>
          Check again
        </Button>
      }
    >
      This app hands playback to mpv rather than bundling its own player, which
      is what lets it play the raw MPEG-TS streams IPTV providers serve. Install
      it with{" "}
      <code className="rounded bg-[var(--muted)] px-1.5 py-0.5">
        brew install mpv
      </code>
      , then check again.
    </Notice>
  );
}

export function AddProvider({ onAdded }: { onAdded?: (id: number) => void }) {
  const qc = useQueryClient();
  const [form, setForm] = useState({
    name: "",
    baseUrl: "",
    username: "",
    password: "",
  });

  const add = useMutation({
    mutationFn: api.addProvider,
    onSuccess: async (id) => {
      await qc.invalidateQueries({ queryKey: ["providers"] });
      setForm({ name: "", baseUrl: "", username: "", password: "" });
      onAdded?.(id);
    },
  });

  const field = (key: keyof typeof form) => ({
    value: form[key],
    onChange: (e: React.ChangeEvent<HTMLInputElement>) =>
      setForm((f) => ({ ...f, [key]: e.target.value })),
  });

  return (
    <Card className="w-full max-w-md p-5">
      <h2 className="text-base font-semibold">Add a provider</h2>
      <Muted className="mt-1">
        Credentials are verified before anything is saved, and the password goes
        straight to the macOS Keychain.
      </Muted>

      <form
        className="mt-5 space-y-3"
        onSubmit={(e) => {
          e.preventDefault();
          add.mutate(form);
        }}
      >
        <label className="block space-y-1.5">
          <span className="text-xs font-medium">Name</span>
          <Input placeholder="Living room" required {...field("name")} />
        </label>
        <label className="block space-y-1.5">
          <span className="text-xs font-medium">Server URL</span>
          <Input
            placeholder="http://example.com:8080"
            required
            autoCapitalize="off"
            autoCorrect="off"
            spellCheck={false}
            {...field("baseUrl")}
          />
        </label>
        <label className="block space-y-1.5">
          <span className="text-xs font-medium">Username</span>
          <Input
            required
            autoCapitalize="off"
            autoCorrect="off"
            spellCheck={false}
            {...field("username")}
          />
        </label>
        <label className="block space-y-1.5">
          <span className="text-xs font-medium">Password</span>
          <Input type="password" required {...field("password")} />
        </label>

        {add.error ? (
          <p className="text-sm text-[var(--destructive)]">
            {errorMessage(add.error)}
          </p>
        ) : null}

        <Button
          type="submit"
          variant="primary"
          className="w-full"
          disabled={add.isPending}
        >
          {add.isPending ? "Checking…" : "Add provider"}
        </Button>
      </form>
    </Card>
  );
}

export function FirstRun() {
  return (
    <div className="flex h-full items-center justify-center p-10">
      <div className="space-y-6">
        <div className="text-center">
          <h1 className="text-2xl font-semibold">Xtream</h1>
          <Muted className="mt-1">
            No ads, no upsells, no telemetry. Point it at your subscription.
          </Muted>
        </div>
        <AddProvider />
      </div>
    </div>
  );
}
