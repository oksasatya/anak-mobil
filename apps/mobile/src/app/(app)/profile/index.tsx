import { Text, View } from "react-native";

import { AmAvatar, AmCard } from "@/components/display";
import { TabScreen } from "@/components/shell";
import { AmSkeleton } from "@/components/state";
import { SignOutConfirm } from "@/features/auth/SignOutConfirm";
import { resolveProfileIdentity } from "@/features/profile/identity";
import { useSession } from "@/shared";
import { useTheme } from "@/theme";

/**
 * Who you are, and the way out.
 *
 * Identity comes from the session the bootstrap gate already fetched — one
 * `GET /me` per launch, not one per screen that wants a name.
 *
 * Signing out is `SignOutConfirm` (Plan B, `features/auth/SignOutConfirm.tsx`):
 * it already owns the confirmation sheet, the double-tap guard, and the one
 * call into `signOut()`. This screen mounts it and nothing else — rebuilding
 * that transaction here is the second sign-out control the TEMPORARY comment
 * on the AM-14 healthcheck screen existed to prevent.
 */
export default function ProfileScreen() {
  const theme = useTheme();
  const { user } = useSession();

  // AppGate means a signed-out person never reaches this screen, so a null
  // user is the moment before the session resolves — a skeleton, not an
  // error.
  if (!user) {
    return (
      <TabScreen>
        <AmCard role="working">
          <View style={{ gap: theme.space[3], alignItems: "center" }}>
            <AmSkeleton height={72} width={72} radius="pill" />
            <AmSkeleton height={24} width="60%" />
            <AmSkeleton height={14} width="40%" />
          </View>
        </AmCard>
      </TabScreen>
    );
  }

  const identity = resolveProfileIdentity(user);
  // The email line adds nothing when it is already the headline — the
  // account has neither a display name nor a username.
  const showEmail = identity.name !== user.email;

  return (
    <TabScreen>
      <Text accessibilityRole="header" style={[theme.type.h1, { color: theme.color.textPrimary }]}>
        Profil
      </Text>

      <AmCard role="working">
        <View style={{ gap: theme.space[3], alignItems: "center" }}>
          <AmAvatar name={identity.name} size={72} />
          <View style={{ gap: theme.space[1], alignItems: "center" }}>
            <Text style={[theme.type.h2, { color: theme.color.textPrimary }]}>{identity.name}</Text>
            {identity.handle ? (
              <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
                {identity.handle}
              </Text>
            ) : null}
            {showEmail ? (
              <Text style={[theme.type.caption, { color: theme.color.textTertiary }]}>
                {user.email}
              </Text>
            ) : null}
          </View>
        </View>
      </AmCard>

      <SignOutConfirm />
    </TabScreen>
  );
}
