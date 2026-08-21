export interface ProfileIdentity {
  readonly name: string;
  readonly handle: string | null;
}

/** An empty or whitespace-only string reads the same as absent. */
function normalize(value: string | null): string | null {
  const trimmed = value?.trim();
  return trimmed ? trimmed : null;
}

/**
 * The name and handle line for the Profile tab's header.
 *
 * `displayName` and `username` are both nullable on the wire (`Me`, Plan A),
 * and a naive `??` chain stops at null/undefined only — an empty-string
 * value would pass straight through, printing a blank name or a bare "@"
 * handle. `normalize` treats both the same as absent.
 *
 * The handle line is shown only when it says something the name line does
 * not: a username distinct from whatever ended up as the name. Where the
 * name IS the username (no display name set), repeating it as "@username"
 * underneath itself is noise, not information.
 */
export function resolveProfileIdentity(user: {
  readonly displayName: string | null;
  readonly username: string | null;
  readonly email: string;
}): ProfileIdentity {
  const displayName = normalize(user.displayName);
  const username = normalize(user.username);
  const name = displayName ?? username ?? user.email;
  const handle = username && username !== name ? `@${username}` : null;
  return { name, handle };
}
