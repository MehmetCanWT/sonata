import { Monitor, Smartphone, Terminal } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { SiteStrings } from "@/i18n";

interface ClientsSectionProps {
  t: SiteStrings;
}

export function ClientsSection({ t }: ClientsSectionProps) {
  return (
    <section
      id="clients"
      className="border-t border-border bg-card/35 py-20 lg:py-24"
    >
      <div className="mx-auto flex w-full max-w-6xl flex-col gap-10 px-6">
        <div className="flex max-w-2xl flex-col gap-3">
          <h2 className="text-3xl font-semibold tracking-tight text-foreground sm:text-4xl">
            {t.clients.heading}
          </h2>
          <p className="text-base leading-relaxed text-muted-foreground">
            {t.clients.lead}
          </p>
        </div>

        <Tabs defaultValue="desktop" className="w-full">
          <TabsList aria-label={t.clients.heading}>
            <TabsTrigger value="desktop">
              <Monitor />
              {t.clients.tabs.desktop}
            </TabsTrigger>
            <TabsTrigger value="tui">
              <Terminal />
              {t.clients.tabs.tui}
            </TabsTrigger>
            <TabsTrigger value="android">
              <Smartphone />
              {t.clients.tabs.android}
            </TabsTrigger>
          </TabsList>

          <TabsContent value="desktop">
            <div className="grid items-start gap-8 lg:grid-cols-12">
              <div className="tactile-card min-w-0 overflow-hidden rounded-xl border border-border bg-card lg:col-span-7">
                <img
                  src="./media/sonata-desktop.webp"
                  alt={t.clients.desktopPanel.imageAlt}
                  width={1200}
                  height={675}
                  loading="lazy"
                  className="art-zoom block aspect-video w-full object-cover"
                />
              </div>

              <Card className="min-w-0 lg:col-span-5">
                <CardHeader>
                  <CardTitle>{t.clients.desktopPanel.title}</CardTitle>
                  <CardDescription>{t.clients.desktopPanel.summary}</CardDescription>
                </CardHeader>
                <CardContent className="flex flex-col gap-3">
                  {t.clients.desktopPanel.specs.map((spec) => (
                    <div
                      key={spec.label}
                      className="flex flex-col gap-1 border-t border-border pt-3"
                    >
                      <span className="text-xs font-medium text-muted-foreground">
                        {spec.label}
                      </span>
                      <span className="text-sm text-foreground">{spec.value}</span>
                    </div>
                  ))}
                </CardContent>
              </Card>
            </div>
          </TabsContent>

          <TabsContent value="tui">
            <div className="grid items-start gap-8 lg:grid-cols-12">
              <div className="tactile-card min-w-0 overflow-hidden rounded-xl border border-border bg-card lg:col-span-7">
                <img
                  src="./media/sonata-tui.webp"
                  alt={t.clients.tuiPanel.imageAlt}
                  width={1200}
                  height={675}
                  loading="lazy"
                  className="art-zoom block aspect-video w-full object-cover"
                />
              </div>

              <Card className="min-w-0 lg:col-span-5">
                <CardHeader>
                  <CardTitle>{t.clients.tuiPanel.title}</CardTitle>
                  <CardDescription>{t.clients.tuiPanel.summary}</CardDescription>
                </CardHeader>
                <CardContent className="flex flex-col gap-3">
                  {t.clients.tuiPanel.specs.map((spec) => (
                    <div
                      key={spec.label}
                      className="flex flex-col gap-1 border-t border-border pt-3"
                    >
                      <span className="text-xs font-medium text-muted-foreground">
                        {spec.label}
                      </span>
                      <span className="text-sm text-foreground">{spec.value}</span>
                    </div>
                  ))}
                </CardContent>
              </Card>
            </div>
          </TabsContent>

          <TabsContent value="android">
            <div className="grid items-start gap-8 lg:grid-cols-12">
              <Card className="min-w-0 lg:col-span-7">
                <CardHeader>
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <CardTitle>{t.clients.androidPanel.title}</CardTitle>
                    <Badge variant="brand">arm64-v8a</Badge>
                  </div>
                  <CardDescription>{t.clients.androidPanel.summary}</CardDescription>
                </CardHeader>
                <CardContent className="flex flex-col gap-4">
                  <div className="rounded-lg border border-border bg-secondary/60 px-4 py-3 text-sm text-muted-foreground">
                    {t.clients.androidPanel.architectureNote}
                  </div>
                  <div className="grid gap-4 sm:grid-cols-3">
                    {t.clients.androidPanel.highlights.map((item) => (
                      <div
                        key={item.title}
                        className="flex flex-col gap-1.5 rounded-lg border border-border bg-background/60 p-4"
                      >
                        <span className="text-sm font-medium text-foreground">
                          {item.title}
                        </span>
                        <span className="text-xs leading-relaxed text-muted-foreground">
                          {item.detail}
                        </span>
                      </div>
                    ))}
                  </div>
                </CardContent>
              </Card>

              <Card className="min-w-0 lg:col-span-5">
                <CardHeader>
                  <CardTitle>{t.clients.tabs.android}</CardTitle>
                </CardHeader>
                <CardContent className="flex flex-col gap-3">
                  {t.clients.androidPanel.specs.map((spec) => (
                    <div
                      key={spec.label}
                      className="flex flex-col gap-1 border-t border-border pt-3"
                    >
                      <span className="text-xs font-medium text-muted-foreground">
                        {spec.label}
                      </span>
                      <span className="text-sm text-foreground">{spec.value}</span>
                    </div>
                  ))}
                </CardContent>
              </Card>
            </div>
          </TabsContent>
        </Tabs>

        <Separator />

        <div className="flex min-w-0 flex-col gap-4">
          <h3 className="text-lg font-semibold text-foreground">
            {t.clients.matrixTitle}
          </h3>
          <div className="w-full overflow-x-auto rounded-xl border border-border bg-card">
            <table className="w-full min-w-[640px] border-collapse text-left text-sm">
              <thead>
                <tr className="border-b border-border bg-secondary/50 text-xs font-medium text-muted-foreground">
                  {t.clients.matrixHeaders.map((header) => (
                    <th key={header} className="px-4 py-3.5 font-medium">
                      {header}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {t.clients.matrixRows.map((row) => (
                  <tr
                    key={row[0]}
                    className="border-b border-border last:border-b-0"
                  >
                    <th scope="row" className="px-4 py-3.5 font-medium text-foreground">
                      {row[0]}
                    </th>
                    <td className="px-4 py-3.5 text-muted-foreground">{row[1]}</td>
                    <td className="px-4 py-3.5 text-muted-foreground">{row[2]}</td>
                    <td className="px-4 py-3.5 text-muted-foreground">{row[3]}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </section>
  );
}
