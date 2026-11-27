import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { getStrings } from "@/lib/i18n";
import { useSonataStore } from "@/store/useSonataStore";
import type { ServerKind } from "@/types/sonata";

export function Modals() {
  const snapshot = useSonataStore((s) => s.snapshot);
  const activeModal = useSonataStore((s) => s.activeModal);
  const setModal = useSonataStore((s) => s.setModal);
  const dispatch = useSonataStore((s) => s.dispatch);

  const [playlistName, setPlaylistName] = useState("");
  const [serverName, setServerName] = useState("");
  const [serverKind, setServerKind] = useState<ServerKind>("Jellyfin");
  const [serverUrl, setServerUrl] = useState("");
  const [serverUser, setServerUser] = useState("");
  const [serverSecret, setServerSecret] = useState("");
  const [allowInsecureLanHttp, setAllowInsecureLanHttp] = useState(false);

  const t = getStrings(snapshot.config.language);
  const isHttpServerUrl =
    serverKind !== "Smb" && serverUrl.trim().toLowerCase().startsWith("http://");

  if (!activeModal) return null;

  return (
    <Dialog open={true} onOpenChange={(open) => !open && setModal(null)}>
      <DialogContent>
        {activeModal.type === "login" && (
          <>
            <DialogHeader>
              <DialogTitle>{t.signInYtmTitle}</DialogTitle>
              <DialogDescription>{t.signInYtmDesc}</DialogDescription>
            </DialogHeader>
            {snapshot.sign_in_error && (
              <div className="mb-3 rounded-lg border border-red-500/30 bg-red-500/10 p-2.5 text-xs text-red-300">
                {snapshot.sign_in_error}
              </div>
            )}
            <div className="space-y-2 max-h-72 overflow-y-auto sonata-scroll">
              {snapshot.available_browsers.map((b) => (
                <button
                  key={b.id}
                  type="button"
                  disabled={snapshot.signing_in}
                  onClick={() => void dispatch({ type: "SignInWithBrowser", browser_id: b.id })}
                  className="flex w-full items-center justify-between rounded-xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-3.5 py-2.5 text-sm hover:bg-[var(--bg-hover)] disabled:opacity-50"
                >
                  <span className="font-medium">{b.label}</span>
                  <span className="text-xs text-[var(--ink-muted)]">
                    {b.detected ? t.detected : t.notDetected}
                  </span>
                </button>
              ))}
            </div>
          </>
        )}

        {activeModal.type === "createPlaylist" && (
          <>
            <DialogHeader>
              <DialogTitle>
                {activeModal.kind === "ytm"
                  ? t.createYtmPlaylistTitle
                  : t.createLocalPlaylistTitle}
              </DialogTitle>
            </DialogHeader>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                const name = playlistName.trim();
                if (!name) return;
                void dispatch({
                  type: activeModal.kind === "ytm" ? "CreateYtmPlaylist" : "CreatePersonalPlaylist",
                  name,
                });
                setPlaylistName("");
                setModal(null);
              }}
              className="space-y-4"
            >
              <Input
                autoFocus
                value={playlistName}
                onChange={(e) => setPlaylistName(e.target.value)}
                placeholder={t.playlistNamePlaceholder}
              />
              <div className="flex justify-end gap-2">
                <Button type="button" variant="ghost" onClick={() => setModal(null)}>
                  {t.cancel}
                </Button>
                <Button type="submit" variant="accent">
                  {t.create}
                </Button>
              </div>
            </form>
          </>
        )}

        {activeModal.type === "addToPlaylist" && (
          <>
            <DialogHeader>
              <DialogTitle>{t.addToPlaylist}</DialogTitle>
              <DialogDescription>{activeModal.track.title}</DialogDescription>
            </DialogHeader>
            <div className="space-y-1.5 max-h-72 overflow-y-auto sonata-scroll">
              {snapshot.personal_playlists.map((pl) => (
                <button
                  key={pl.id}
                  type="button"
                  onClick={() => {
                    void dispatch({
                      type: "AddTrackToPersonalPlaylist",
                      track: activeModal.track,
                      playlist_id: pl.id,
                    });
                    setModal(null);
                  }}
                  className="flex w-full items-center justify-between rounded-xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-3.5 py-2 text-xs font-medium hover:bg-[var(--bg-hover)]"
                >
                  <span>{pl.name}</span>
                  <span className="text-[var(--ink-muted)]">{t.localTag}</span>
                </button>
              ))}
              {snapshot.ytm_playlists
                .filter((p) => p.is_owned && p.id !== "VLLM")
                .map((pl) => (
                  <button
                    key={pl.id}
                    type="button"
                    onClick={() => {
                      void dispatch({
                        type: "AddTrackToPlaylist",
                        track: activeModal.track,
                        playlist_id: pl.id,
                        playlist_name: pl.name,
                      });
                      setModal(null);
                    }}
                    className="flex w-full items-center justify-between rounded-xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-3.5 py-2 text-xs font-medium hover:bg-[var(--bg-hover)]"
                  >
                    <span>{pl.name}</span>
                    <span className="text-[var(--ink-muted)]">{t.ytmTag}</span>
                  </button>
                ))}
            </div>
          </>
        )}

        {activeModal.type === "sleepTimer" && (
          <>
            <DialogHeader>
              <DialogTitle>{t.sleepTimer}</DialogTitle>
              <DialogDescription>
                {snapshot.sleep_timer_remaining_secs != null
                  ? `${Math.ceil(snapshot.sleep_timer_remaining_secs / 60)} ${t.minutesRemainingSuffix}`
                  : t.sleepTimerDesc}
              </DialogDescription>
            </DialogHeader>
            <div className="grid grid-cols-3 gap-2">
              {[15, 30, 45, 60, 90].map((mins) => (
                <Button
                  key={mins}
                  variant="secondary"
                  onClick={() => {
                    void dispatch({ type: "SetSleepTimer", minutes: mins });
                    setModal(null);
                  }}
                >
                  {mins}
                  {t.minutesShortSuffix}
                </Button>
              ))}
              <Button
                variant="destructive"
                onClick={() => {
                  void dispatch({ type: "SetSleepTimer", minutes: null });
                  setModal(null);
                }}
              >
                {t.offLabel}
              </Button>
            </div>
          </>
        )}

        {activeModal.type === "serverEditor" && (
          <>
            <DialogHeader>
              <DialogTitle>{t.addMusicServerTitle}</DialogTitle>
            </DialogHeader>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                if (!serverName.trim() || !serverUrl.trim()) return;
                void dispatch({
                  type: "SaveServerProfile",
                  name: serverName.trim(),
                  kind: serverKind,
                  base_url: serverUrl.trim(),
                  username: serverUser.trim() || null,
                  password: serverSecret || null,
                  allow_insecure_lan_http: isHttpServerUrl && allowInsecureLanHttp,
                });
                setServerName("");
                setServerUrl("");
                setServerUser("");
                setServerSecret("");
                setAllowInsecureLanHttp(false);
                setModal(null);
              }}
              className="space-y-3"
            >
              <Select value={serverKind} onValueChange={(v) => setServerKind(v as ServerKind)}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="Jellyfin">Jellyfin</SelectItem>
                  <SelectItem value="Subsonic">Subsonic / Navidrome</SelectItem>
                  <SelectItem value="WebDav">WebDAV</SelectItem>
                  <SelectItem value="Smb">{t.serverTypeSmb}</SelectItem>
                </SelectContent>
              </Select>
              <Input
                value={serverName}
                onChange={(e) => setServerName(e.target.value)}
                placeholder={t.serverNamePlaceholder}
              />
              <Input
                value={serverUrl}
                onChange={(e) => setServerUrl(e.target.value)}
                placeholder={t.serverUrlPlaceholder}
              />
              <Input
                value={serverUser}
                onChange={(e) => setServerUser(e.target.value)}
                placeholder={t.serverUserPlaceholder}
              />
              <Input
                type="password"
                value={serverSecret}
                onChange={(e) => setServerSecret(e.target.value)}
                placeholder={t.serverSecretPlaceholder}
              />
              {isHttpServerUrl && (
                <div className="space-y-2 rounded-xl border border-amber-500/30 bg-amber-500/10 p-3 text-xs">
                  <p className="text-amber-200">{t.allowInsecureLanHttpWarning}</p>
                  <label className="flex items-center gap-2 font-medium text-[var(--ink-primary)] cursor-pointer select-none">
                    <input
                      type="checkbox"
                      checked={allowInsecureLanHttp}
                      onChange={(e) => setAllowInsecureLanHttp(e.target.checked)}
                      className="h-4 w-4 accent-[var(--accent)]"
                    />
                    <span>{t.allowInsecureLanHttpLabel}</span>
                  </label>
                </div>
              )}
              <div className="flex justify-end gap-2 pt-1">
                <Button type="button" variant="ghost" onClick={() => setModal(null)}>
                  {t.cancel}
                </Button>
                <Button type="submit" variant="accent">
                  {t.save}
                </Button>
              </div>
            </form>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
