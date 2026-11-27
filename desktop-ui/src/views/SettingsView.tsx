import { ExternalLink, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { getStrings } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { useSonataStore } from "@/store/useSonataStore";
import type { AppLanguage, ThemeModePref } from "@/types/sonata";

export function isSafeReleaseUrl(raw?: string | null): boolean {
  if (!raw) return false;
  try {
    const parsed = new URL(raw.trim());
    if (parsed.protocol !== "https:") return false;
    if (parsed.username || parsed.password) return false;
    return parsed.hostname.toLowerCase() === "github.com";
  } catch {
    return false;
  }
}

function SettingRow({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col @sm:flex-row @sm:items-center justify-between gap-3 py-3 border-b border-[var(--border-hairline)] last:border-b-0">
      <div className="min-w-0">
        <div className="text-sm font-medium text-[var(--ink-primary)]">{label}</div>
        {description && (
          <div className="text-xs text-[var(--ink-secondary)] mt-0.5">{description}</div>
        )}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

export function SettingsView() {
  const snapshot = useSonataStore((s) => s.snapshot);
  const settings = useSonataStore((s) => s.settings);
  const platform = useSonataStore((s) => s.platform);
  const setThemeMode = useSonataStore((s) => s.setThemeMode);
  const updateSettings = useSonataStore((s) => s.updateSettings);
  const setModal = useSonataStore((s) => s.setModal);
  const dispatch = useSonataStore((s) => s.dispatch);

  const [connectName, setConnectName] = useState(snapshot.config.connect_device_name);

  useEffect(() => {
    setConnectName(snapshot.config.connect_device_name);
  }, [snapshot.config.connect_device_name]);

  const t = getStrings(snapshot.config.language);

  return (
    <div className="sonata-scroll h-full overflow-y-auto px-5 @md:px-8 pt-5 pb-36">
      <div className="mx-auto max-w-3xl space-y-6">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">{t.settings}</h1>
          <p className="mt-1 text-xs text-[var(--ink-secondary)]">
            {t.desktopPrefsSubtitle} ({platform})
          </p>
        </div>

        {/* Appearance */}
        <section className="rounded-2xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-5 py-2">
          <h2 className="py-3 text-xs font-semibold uppercase tracking-wider text-[var(--ink-muted)]">
            {t.appearance}
          </h2>
          <SettingRow label={t.themeMode} description={t.themeModeDesc}>
            <Select
              value={settings.theme_mode}
              onValueChange={(v) => void setThemeMode(v as ThemeModePref)}
            >
              <SelectTrigger className="w-32">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="dark">{t.darkMode}</SelectItem>
                <SelectItem value="light">{t.lightMode}</SelectItem>
                <SelectItem value="system">{t.systemMode}</SelectItem>
              </SelectContent>
            </Select>
          </SettingRow>

          <SettingRow label={t.languageLabel}>
            <Select
              value={snapshot.config.language}
              onValueChange={(lang) => {
                void dispatch({ type: "SetLanguage", language: lang as AppLanguage });
                void updateSettings({ language: lang as AppLanguage });
              }}
            >
              <SelectTrigger className="w-36">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="en">English</SelectItem>
                <SelectItem value="tr">Türkçe</SelectItem>
                <SelectItem value="ru">Русский</SelectItem>
              </SelectContent>
            </Select>
          </SettingRow>

          <SettingRow
            label={t.adaptiveArtColor}
            description={t.adaptiveArtColorDesc}
          >
            <Switch
              checked={settings.adaptive_art_color}
              onCheckedChange={(v) => void updateSettings({ adaptive_art_color: v })}
            />
          </SettingRow>

          <SettingRow
            label={t.motionCovers}
            description={t.motionCoversDesc}
          >
            <Switch
              checked={snapshot.config.motion_covers}
              onCheckedChange={() => void dispatch({ type: "ToggleMotionCovers" })}
            />
          </SettingRow>

          <SettingRow label={t.reduceMotion}>
            <Switch
              checked={snapshot.config.reduce_motion}
              onCheckedChange={() => void dispatch({ type: "ToggleReduceMotion" })}
            />
          </SettingRow>
        </section>

        {/* Playback & Audio */}
        <section className="rounded-2xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-5 py-2">
          <h2 className="py-3 text-xs font-semibold uppercase tracking-wider text-[var(--ink-muted)]">
            {t.playbackAndAudio}
          </h2>
          <SettingRow
            label={t.hiFiSubstitution}
            description={t.hiFiSubstitutionDesc}
          >
            <Switch
              checked={snapshot.config.hifi_substitution}
              onCheckedChange={() => void dispatch({ type: "ToggleHiFi" })}
            />
          </SettingRow>

          <SettingRow label={t.jiosaavnFallback}>
            <Switch
              checked={snapshot.config.jiosaavn_enabled}
              onCheckedChange={() => void dispatch({ type: "ToggleJioSaavn" })}
            />
          </SettingRow>

          <SettingRow label={t.gaplessPlayback}>
            <Switch
              checked={snapshot.config.gapless_enabled}
              onCheckedChange={() => void dispatch({ type: "ToggleGapless" })}
            />
          </SettingRow>

          <SettingRow
            label={t.crossfade}
            description={`${snapshot.config.crossfade_secs}${t.crossfadeSuffix}`}
          >
            <div className="w-36">
              <Slider
                value={[snapshot.config.crossfade_secs]}
                min={0}
                max={12}
                step={1}
                onValueChange={([secs]) => void dispatch({ type: "SetCrossfade", secs })}
              />
            </div>
          </SettingRow>

          <SettingRow
            label={t.playbackSpeed}
            description={`${snapshot.playback_speed.toFixed(2)}x`}
          >
            <div className="w-36">
              <Slider
                value={[Math.round(snapshot.playback_speed * 100)]}
                min={50}
                max={200}
                step={5}
                onValueChange={([pct]) =>
                  void dispatch({ type: "SetPlaybackSpeed", speed: pct / 100 })
                }
              />
            </div>
          </SettingRow>

          <SettingRow label={t.skipSilence}>
            <Switch
              checked={snapshot.skip_silence}
              onCheckedChange={() => void dispatch({ type: "ToggleSkipSilence" })}
            />
          </SettingRow>

          <SettingRow
            label={t.personalOnlyMode}
            description={t.personalOnlySettingsDesc}
          >
            <Switch
              checked={snapshot.config.personal_only_mode}
              onCheckedChange={() => void dispatch({ type: "TogglePersonalOnlyMode" })}
            />
          </SettingRow>
        </section>

        {/* Accounts & Integrations */}
        <section className="rounded-2xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-5 py-2">
          <h2 className="py-3 text-xs font-semibold uppercase tracking-wider text-[var(--ink-muted)]">
            {t.accountsAndSync}
          </h2>
          <SettingRow
            label={t.ytmSession}
            description={
              snapshot.signed_in_browser
                ? `${t.signedInVia} ${snapshot.signed_in_browser}`
                : t.notSignedIn
            }
          >
            {snapshot.signed_in_browser ? (
              <Button size="sm" variant="secondary" onClick={() => void dispatch({ type: "SignOut" })}>
                {t.signOut}
              </Button>
            ) : (
              <Button size="sm" variant="accent" onClick={() => setModal({ type: "login" })}>
                {t.signIn}
              </Button>
            )}
          </SettingRow>

          <SettingRow
            label={t.discordPresence}
            description={`${t.discordDesc} (${snapshot.discord_status})`}
          >
            <div className="flex items-center gap-2">
              <Badge>{snapshot.discord_status}</Badge>
              <Button
                size="sm"
                variant={snapshot.config.discord_rpc_enabled ? "secondary" : "accent"}
                onClick={() => void dispatch({ type: "ToggleDiscordRpc" })}
              >
                {snapshot.config.discord_rpc_enabled ? t.disconnectDiscord : t.connectDiscord}
              </Button>
            </div>
          </SettingRow>

          <SettingRow
            label={t.sonataConnect}
            description={
              snapshot.connect_running
                ? `${t.connectActiveOnPort} ${snapshot.connect_http_port ?? ""} • ${t.pinLabel}: ${snapshot.connect_pin ?? "------"}`
                : t.connectInactiveDesc
            }
          >
            <Switch
              checked={snapshot.connect_running}
              onCheckedChange={() => void dispatch({ type: "ToggleConnect" })}
            />
          </SettingRow>

          {snapshot.connect_error && (
            <div className="my-2 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-300">
              {snapshot.connect_error}
            </div>
          )}

          {snapshot.connect_running && snapshot.connect_pending_pair && (
            <SettingRow
              label={t.pendingPairingRequest}
              description={`${snapshot.connect_pending_pair.device_name} ${t.connectPairRequestDesc}`}
            >
              <div className="flex items-center gap-2">
                <Button
                  size="sm"
                  variant="secondary"
                  onClick={() =>
                    void dispatch({
                      type: "RejectConnectPair",
                      request_id: snapshot.connect_pending_pair!.request_id,
                    })
                  }
                >
                  {t.reject}
                </Button>
                <Button
                  size="sm"
                  variant="accent"
                  onClick={() =>
                    void dispatch({
                      type: "ApproveConnectPair",
                      request_id: snapshot.connect_pending_pair!.request_id,
                    })
                  }
                >
                  {t.approve}
                </Button>
              </div>
            </SettingRow>
          )}

          {snapshot.connect_running && (
            <>
              <SettingRow label={t.connectDeviceName}>
                <div className="flex items-center gap-2">
                  <Input
                    value={connectName}
                    onChange={(e) => setConnectName(e.target.value)}
                    className="w-40 h-8 text-xs"
                  />
                  <Button
                    size="sm"
                    variant="secondary"
                    onClick={() =>
                      void dispatch({ type: "SetConnectDeviceName", name: connectName })
                    }
                  >
                    {t.save}
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => void dispatch({ type: "RegenerateConnectPin" })}
                  >
                    {t.newPin}
                  </Button>
                </div>
              </SettingRow>

              <SettingRow
                label={t.connectPairedDevices}
                description={
                  snapshot.connect_paired_devices.length === 0
                    ? t.connectNoPairedDevices
                    : undefined
                }
              >
                {snapshot.connect_paired_devices.length > 0 && (
                  <div className="flex flex-col items-end gap-1.5">
                    {snapshot.connect_paired_devices.map((dev) => (
                      <div key={dev.device_id} className="flex items-center gap-2 text-xs">
                        <span className="text-[var(--ink-primary)]">{dev.device_name}</span>
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={() =>
                            void dispatch({
                              type: "UnpairConnectDevice",
                              device_id: dev.device_id,
                            })
                          }
                        >
                          {t.unpair}
                        </Button>
                      </div>
                    ))}
                  </div>
                )}
              </SettingRow>
            </>
          )}
        </section>

        {/* Storage & Updates */}
        <section className="rounded-2xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-5 py-2">
          <h2 className="py-3 text-xs font-semibold uppercase tracking-wider text-[var(--ink-muted)]">
            {t.libraryAndStorage} & {t.aboutAndUpdates}
          </h2>
          <SettingRow
            label={t.mediaCacheLimit}
            description={`${snapshot.config.cache_limit_gb} GB`}
          >
            <div className="flex items-center gap-1.5">
              <Button size="sm" variant="secondary" onClick={() => void dispatch({ type: "StepCacheLimit", up: false })}>
                -
              </Button>
              <Button size="sm" variant="secondary" onClick={() => void dispatch({ type: "StepCacheLimit", up: true })}>
                +
              </Button>
            </div>
          </SettingRow>

          <SettingRow
            label={t.checkUpdates}
            description={
              snapshot.update_info
                ? snapshot.update_info.update_available
                  ? `${t.updateAvailable} v${snapshot.update_info.latest_version}`
                  : `${t.upToDate} (v${snapshot.update_info.current_version})`
                : t.githubReleases
            }
          >
            <div className="flex items-center gap-2">
              {snapshot.update_info?.update_available && (
                <Badge>v{snapshot.update_info.latest_version}</Badge>
              )}
              <Button
                size="sm"
                variant="secondary"
                onClick={() => void dispatch({ type: "CheckForUpdates" })}
              >
                <RefreshCw className={cn("h-3.5 w-3.5", snapshot.checking_updates && "animate-spin")} />
                {t.checkUpdates}
              </Button>
              {snapshot.update_info?.update_available &&
                isSafeReleaseUrl(snapshot.update_info.release_url) && (
                <a
                  href={snapshot.update_info.release_url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex h-8 items-center gap-1 rounded-lg border border-[var(--border-hairline)] px-2.5 text-xs hover:bg-[var(--bg-hover)]"
                >
                  <ExternalLink className="h-3.5 w-3.5" />
                  {t.releaseLink}
                </a>
              )}
            </div>
          </SettingRow>
        </section>
      </div>
    </div>
  );
}
