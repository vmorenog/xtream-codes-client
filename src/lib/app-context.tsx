import { createContext, useContext } from "react";

import type { PlayableRef, Provider } from "@/lib/api";

interface AppContextValue {
  provider: Provider;
  play: (ref: PlayableRef) => void;
}

export const AppContext = createContext<AppContextValue | null>(null);

/** Available to every route; the root gate guarantees a **Provider** exists. */
export function useApp() {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error("useApp used outside the app shell");
  return ctx;
}
