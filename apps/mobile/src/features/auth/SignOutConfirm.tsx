import { useState } from "react";
import { Text, View } from "react-native";

import { AmBottomSheet } from "@/components/display";
import { AmButton } from "@/components/input";
import { signOut } from "@/shared";
import { useTheme } from "@/theme";

export interface SignOutConfirmProps {
  readonly label?: string;
  /**
   * How the TRIGGER looks. The confirmation inside the sheet is always the
   * filled `destructive` — that is where the alarm belongs. On the Profile
   * tab the trigger is `destructive-quiet`, because a filled red block is not
   * what a permanently-visible "Keluar" should look like.
   */
  readonly variant?: "destructive" | "destructive-quiet";
}

/**
 * The button and its confirmation. Nothing more.
 *
 * signOut() is Plan A's transaction — epoch, cancellation, cache, secure
 * store, one redirect. This component's entire job is to ask once and call it
 * once. It clears nothing, deletes nothing, and navigates nowhere itself.
 */
export function SignOutConfirm({ label = "Keluar", variant = "destructive" }: SignOutConfirmProps) {
  const theme = useTheme();
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);

  const confirm = () => {
    // NO `if (busy) return;` guard here, and that is deliberate — the one that
    // used to sit here was unreachable on every path. Once the commit lands,
    // `AmButton` computes `inert = disabled || loading` and passes it to
    // `Pressable disabled`, so `loading={busy}` stops the tap before this runs;
    // and a second tap landing BEFORE the commit reads the previous render's
    // closure, where `busy` is still `false`, so the check would not have
    // stopped it either. What actually makes the AC true is `signOut()`'s
    // module-level single-flight (`inFlight ??= run()`): any number of calls
    // collapse into one `run()` and one `POST /auth/logout`. Found in Task 6's
    // review.
    setBusy(true);
    void signOut()
      // `.finally()` alone was not enough: it returns a promise that ADOPTS the
      // rejection, and `void` then discards it — an unhandled rejection, which
      // is the exact shape (`void signOut()`) this component was written to
      // replace. `signOut` really can reject; its own body is try/finally with
      // no catch. The person is signed out either way, because `setSignedOut()`
      // runs in that finally, so there is nothing to tell them and nothing to
      // retry.
      .catch(() => {})
      .finally(() => {
        setBusy(false);
        setOpen(false);
      });
  };

  return (
    <>
      <AmButton label={label} variant={variant} onPress={() => setOpen(true)} />
      <AmBottomSheet
        visible={open}
        // Guarded by `busy`. "Batal" is already disabled mid-transaction, but
        // the sheet has four other ways out — the scrim, the "Tutup" header
        // button, a drag down, and Android's hardware back — all of which
        // routed here unguarded. Dismissing that way left `busy` true with the
        // sheet closed, so reopening it showed two greyed-out buttons until the
        // transaction settled. Found in Task 6's review.
        onClose={() => {
          if (!busy) setOpen(false);
        }}
        title="Keluar dari akun?"
      >
        <View style={{ gap: theme.space[4], paddingBottom: theme.space[4] }}>
          <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
            Data garasi kamu tetap aman di akun ini. Kamu perlu masuk lagi untuk membukanya.
          </Text>
          <AmButton label="Ya, keluar" variant="destructive" loading={busy} onPress={confirm} />
          <AmButton
            label="Batal"
            variant="secondary"
            disabled={busy}
            onPress={() => setOpen(false)}
          />
        </View>
      </AmBottomSheet>
    </>
  );
}
