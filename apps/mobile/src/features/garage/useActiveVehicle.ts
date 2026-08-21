import { useEffect } from "react";

import { setActiveVehicleId, useActiveVehicleId } from "@/shared";

import type { Vehicle } from "./types";

/**
 * The car every screen means by "mobil aktif".
 *
 * The stored id is a preference, not an authority. A car can be removed on
 * another device, and a screen that trusted the id alone would then render
 * nothing at all while the person is looking at a garage that has cars in it.
 * So the list decides, and the store is healed to match.
 *
 * The healing happens in an effect rather than during render, because a store
 * write during render is how a component re-renders forever.
 */
export function useActiveVehicle(vehicles: readonly Vehicle[] | undefined): Vehicle | null {
  const storedId = useActiveVehicleId();
  const active = vehicles?.find((vehicle) => vehicle.id === storedId) ?? vehicles?.[0] ?? null;

  useEffect(() => {
    if (active && active.id !== storedId) setActiveVehicleId(active.id);
  }, [active, storedId]);

  return active;
}
