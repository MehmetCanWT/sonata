import { useState } from "react";
import {
  ArrowUpRight,
  Check,
  Code2,
  Copy,
  Download,
  Laptop,
  Monitor,
  Smartphone,
  Terminal,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { PlatformId, SiteStrings } from "@/i18n";
import { detectPlatform, type ReleaseInfo } from "@/lib/release";

interface DownloadSectionProps {
  t: SiteStrings;
  release: ReleaseInfo;
}

function CommandBlock({
  label,
  command,
  copyLabel,
  copiedLabel,
}: {
  label: string;
  command: string;
  copyLabel: string;
  copiedLabel: string;
}) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(command);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1800);
    } catch {
      // Ignore clipboard permission errors.
    }
  };

  return (
    <div className="flex min-w-0 flex-col gap-2">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="text-xs font-medium text-muted-foreground">{label}</span>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={handleCopy}
          aria-label={`${copyLabel}: ${label}`}
        >
          {copied ? (
            <Check data-icon="inline-start" />
          ) : (
            <Copy data-icon="inline-start" />
          )}
          {copied ? copiedLabel : copyLabel}
        </Button>
      </div>
      <pre className="max-w-full overflow-x-auto rounded-lg border border-border bg-background px-4 py-3 font-mono text-xs leading-relaxed text-foreground">
        <code>{command}</code>
      </pre>
    </div>
  );
}

export function DownloadSection({ t, release }: DownloadSectionProps) {
  const [platform, setPlatform] = useState<PlatformId>(() => detectPlatform());

  const desktopCurl =
    "curl -fsSL https://raw.githubusercontent.com/MehmetCanWT/sonata/main/install-desktop.sh | sh";
  const tuiCurl =
    "curl -fsSL https://raw.githubusercontent.com/MehmetCanWT/sonata/main/install.sh | sh";
  const obtainiumRepo = "https://github.com/MehmetCanWT/sonata";
  const obtainiumFilter = "Sonata-.*-arm64\\.apk$";

  const rustBuildCmd =
    "cargo build --release --locked -p sonata-tui -p sonata-desktop";
  const androidBuildCmd = [
    "rustup target add aarch64-linux-android",
    "cargo ndk -t arm64-v8a -P 31 -o android/app/src/main/jniLibs build -p sonata-android --release --locked",
    "cd android && ./gradlew :app:assembleRelease --console=plain",
  ].join("\n");
  const verifyCmd = [
    "./scripts/check_boundaries.sh",
    "cargo fmt --all -- --check",
    "cargo clippy --workspace --all-targets --all-features --locked -- -D warnings",
    "cargo test --offline -p sonata-core -p sonata-runtime -p sonata-desktop -p sonata-tui -p sonata-android",
  ].join("\n");

  return (
    <section id="download" className="border-t border-border py-20 lg:py-24">
      <div className="mx-auto flex w-full max-w-6xl flex-col gap-10 px-6">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div className="flex max-w-2xl flex-col gap-3">
            <div className="flex items-center gap-2.5">
              <h2 className="text-3xl font-semibold tracking-tight text-foreground sm:text-4xl">
                {t.download.heading}
              </h2>
              <Badge variant="brand">{release.tag}</Badge>
            </div>
            <p className="text-base leading-relaxed text-muted-foreground">
              {t.download.lead}
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-3">
            <Button variant="outline" asChild>
              <a
                href={release.checksumsUrl}
                target="_blank"
                rel="noopener noreferrer"
              >
                {t.download.checksumsLabel}
                <ArrowUpRight data-icon="inline-end" />
              </a>
            </Button>
            <Button variant="ghost" asChild>
              <a
                href={release.releaseUrl}
                target="_blank"
                rel="noopener noreferrer"
              >
                {t.download.allReleasesLabel}
                <ArrowUpRight data-icon="inline-end" />
              </a>
            </Button>
          </div>
        </div>

        <Tabs
          value={platform}
          onValueChange={(value) => setPlatform(value as PlatformId)}
          className="w-full"
        >
          <TabsList aria-label={t.download.heading}>
            <TabsTrigger value="linux">
              <Terminal />
              {t.download.tabs.linux}
            </TabsTrigger>
            <TabsTrigger value="android">
              <Smartphone />
              {t.download.tabs.android}
            </TabsTrigger>
            <TabsTrigger value="macos">
              <Laptop />
              {t.download.tabs.macos}
            </TabsTrigger>
            <TabsTrigger value="windows">
              <Monitor />
              {t.download.tabs.windows}
            </TabsTrigger>
            <TabsTrigger value="source">
              <Code2 />
              {t.download.tabs.source}
            </TabsTrigger>
          </TabsList>

          <TabsContent value="linux">
            <div className="grid gap-6 lg:grid-cols-12">
              <Card className="min-w-0 lg:col-span-6">
                <CardHeader>
                  <CardTitle>{t.download.linux.oneLinersTitle}</CardTitle>
                </CardHeader>
                <CardContent className="flex min-w-0 flex-col gap-5">
                  <CommandBlock
                    label={t.download.linux.desktopInstallLabel}
                    command={desktopCurl}
                    copyLabel={t.download.copyLabel}
                    copiedLabel={t.download.copiedLabel}
                  />
                  <CommandBlock
                    label={t.download.linux.tuiInstallLabel}
                    command={tuiCurl}
                    copyLabel={t.download.copyLabel}
                    copiedLabel={t.download.copiedLabel}
                  />
                </CardContent>
              </Card>

              <Card className="min-w-0 lg:col-span-6">
                <CardHeader>
                  <CardTitle>{t.download.linux.packagesTitle}</CardTitle>
                  <CardDescription>
                    {t.download.linux.packagesNote} {t.download.linux.tuiFlagHint}
                  </CardDescription>
                </CardHeader>
                <CardContent className="flex flex-col gap-4">
                  <div className="grid gap-3 sm:grid-cols-2">
                    <Button variant="default" asChild>
                      <a href={release.assets.linuxAppImage}>
                        <Download data-icon="inline-start" />
                        AppImage (x86_64)
                      </a>
                    </Button>
                    <Button variant="outline" asChild>
                      <a href={release.assets.linuxDeb}>
                        <Download data-icon="inline-start" />
                        Debian / Ubuntu (.deb)
                      </a>
                    </Button>
                    <Button variant="outline" asChild>
                      <a href={release.assets.linuxRpm}>
                        <Download data-icon="inline-start" />
                        Fedora / RPM (.rpm)
                      </a>
                    </Button>
                    <Button variant="outline" asChild>
                      <a href={release.assets.linuxTarGz}>
                        <Download data-icon="inline-start" />
                        Archive (.tar.gz)
                      </a>
                    </Button>
                  </div>

                  <Separator />

                  <div className="flex flex-col gap-2">
                    <span className="text-xs font-medium text-muted-foreground">
                      {t.download.linux.distrosLabel}
                    </span>
                    <div className="flex flex-col gap-2">
                      {t.download.linux.distrosItems.map((item) => (
                        <a
                          key={item.label}
                          href={item.href}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="flex min-h-11 flex-wrap items-center justify-between gap-2 rounded-lg border border-border bg-background/60 px-3.5 py-2 text-sm transition-colors hover:bg-accent"
                        >
                          <span className="font-medium text-foreground">{item.label}</span>
                          <span className="text-xs text-muted-foreground">{item.detail}</span>
                        </a>
                      ))}
                    </div>
                  </div>
                </CardContent>
              </Card>
            </div>
          </TabsContent>

          <TabsContent value="android">
            <div className="grid gap-6 lg:grid-cols-12">
              <Card className="min-w-0 lg:col-span-6">
                <CardHeader>
                  <CardTitle>{t.download.android.title}</CardTitle>
                  <CardDescription>{t.download.android.description}</CardDescription>
                </CardHeader>
                <CardContent className="flex flex-col gap-4">
                  <div className="flex flex-wrap items-center gap-3">
                    <Button size="lg" asChild className="w-full sm:w-fit">
                      <a href={release.assets.androidApk}>
                        <Download data-icon="inline-start" />
                        {t.download.android.downloadApkCta}
                      </a>
                    </Button>
                    <Badge variant="outline" className="font-mono">
                      Sonata-{release.tag}-arm64.apk
                    </Badge>
                  </div>
                  <p className="text-xs leading-relaxed text-muted-foreground">
                    {t.download.android.updaterNote}
                  </p>
                </CardContent>
              </Card>

              <Card className="min-w-0 lg:col-span-6">
                <CardHeader>
                  <CardTitle>{t.download.android.obtainiumTitle}</CardTitle>
                </CardHeader>
                <CardContent className="flex flex-col gap-4">
                  <CommandBlock
                    label={t.download.android.obtainiumRepoLabel}
                    command={obtainiumRepo}
                    copyLabel={t.download.copyLabel}
                    copiedLabel={t.download.copiedLabel}
                  />
                  <CommandBlock
                    label={t.download.android.obtainiumFilterLabel}
                    command={obtainiumFilter}
                    copyLabel={t.download.copyLabel}
                    copiedLabel={t.download.copiedLabel}
                  />
                </CardContent>
              </Card>
            </div>
          </TabsContent>

          <TabsContent value="macos">
            <Card>
              <CardHeader>
                <CardTitle>{t.download.macos.title}</CardTitle>
                <CardDescription>{t.download.macos.description}</CardDescription>
              </CardHeader>
              <CardContent className="flex flex-wrap gap-3">
                <Button size="lg" asChild>
                  <a href={release.assets.macosDmg}>
                    <Download data-icon="inline-start" />
                    {t.download.macos.dmgLabel}
                  </a>
                </Button>
                <Button variant="outline" size="lg" asChild>
                  <a href={release.assets.macosTarGz}>
                    <Download data-icon="inline-start" />
                    {t.download.macos.tarballLabel}
                  </a>
                </Button>
              </CardContent>
              <CardFooter>
                <span className="text-xs text-muted-foreground">
                  {t.download.macos.requirementsNote}
                </span>
              </CardFooter>
            </Card>
          </TabsContent>

          <TabsContent value="windows">
            <Card>
              <CardHeader>
                <CardTitle>{t.download.windows.title}</CardTitle>
                <CardDescription>{t.download.windows.description}</CardDescription>
              </CardHeader>
              <CardContent className="flex flex-wrap gap-3">
                <Button size="lg" asChild>
                  <a href={release.assets.windowsSetup}>
                    <Download data-icon="inline-start" />
                    {t.download.windows.setupLabel}
                  </a>
                </Button>
                <Button variant="outline" size="lg" asChild>
                  <a href={release.assets.windowsMsi}>
                    <Download data-icon="inline-start" />
                    {t.download.windows.msiLabel}
                  </a>
                </Button>
                <Button variant="outline" size="lg" asChild>
                  <a href={release.assets.windowsExe}>
                    <Download data-icon="inline-start" />
                    {t.download.windows.portableLabel}
                  </a>
                </Button>
              </CardContent>
              <CardFooter>
                <span className="text-xs text-muted-foreground">
                  {t.download.windows.requirementsNote}
                </span>
              </CardFooter>
            </Card>
          </TabsContent>

          <TabsContent value="source">
            <Card>
              <CardHeader>
                <CardTitle>{t.download.source.title}</CardTitle>
                <CardDescription>{t.download.source.description}</CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-5">
                <CommandBlock
                  label={t.download.source.rustLabel}
                  command={rustBuildCmd}
                  copyLabel={t.download.copyLabel}
                  copiedLabel={t.download.copiedLabel}
                />
                <CommandBlock
                  label={t.download.source.androidLabel}
                  command={androidBuildCmd}
                  copyLabel={t.download.copyLabel}
                  copiedLabel={t.download.copiedLabel}
                />
                <CommandBlock
                  label={t.download.source.verifyLabel}
                  command={verifyCmd}
                  copyLabel={t.download.copyLabel}
                  copiedLabel={t.download.copiedLabel}
                />
              </CardContent>
            </Card>
          </TabsContent>
        </Tabs>
      </div>
    </section>
  );
}
