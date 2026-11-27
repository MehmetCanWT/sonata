import { Check, HardDrive, Radio } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { SiteStrings } from "@/i18n";

interface SourcesSectionProps {
  t: SiteStrings;
}

export function SourcesSection({ t }: SourcesSectionProps) {
  return (
    <section
      id="sources"
      className="border-t border-border bg-card/35 py-20 lg:py-24"
    >
      <div className="mx-auto flex w-full max-w-6xl flex-col gap-10 px-6">
        <div className="flex max-w-2xl flex-col gap-3">
          <h2 className="text-3xl font-semibold tracking-tight text-foreground sm:text-4xl">
            {t.sources.heading}
          </h2>
          <p className="text-base leading-relaxed text-muted-foreground">
            {t.sources.lead}
          </p>
        </div>

        <div className="grid items-start gap-8 lg:grid-cols-12">
          <div className="min-w-0 overflow-x-auto rounded-xl border border-border bg-card lg:col-span-7">
            <table className="w-full min-w-[480px] border-collapse text-left text-sm">
              <thead>
                <tr className="border-b border-border bg-secondary/50 text-xs font-medium text-muted-foreground">
                  {t.sources.tableHeaders.map((header) => (
                    <th key={header} className="px-4 py-3.5 font-medium">
                      {header}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {t.sources.protocols.map((protocol) => (
                  <tr
                    key={protocol.name}
                    className="border-b border-border last:border-b-0"
                  >
                    <th scope="row" className="px-4 py-4 font-medium text-foreground">
                      {protocol.name}
                    </th>
                    <td className="px-4 py-4 font-mono text-xs text-muted-foreground">
                      {protocol.transport}
                    </td>
                    <td className="px-4 py-4 text-xs leading-relaxed text-muted-foreground">
                      {protocol.features}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <div className="flex min-w-0 flex-col gap-6 lg:col-span-5">
            <Card>
              <CardHeader>
                <div className="flex items-center justify-between gap-2">
                  <Badge variant="brand">{t.sources.personalMode.badge}</Badge>
                  <HardDrive className="size-4 text-muted-foreground" />
                </div>
                <CardTitle>{t.sources.personalMode.title}</CardTitle>
                <CardDescription>{t.sources.personalMode.description}</CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-2.5">
                {t.sources.personalMode.points.map((point) => (
                  <div key={point} className="flex items-start gap-2.5 text-sm text-foreground">
                    <Check className="mt-0.5 size-4 shrink-0 text-brand" />
                    <span className="text-muted-foreground">{point}</span>
                  </div>
                ))}
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <div className="flex items-center justify-between gap-2">
                  <Badge variant="secondary">{t.sources.cloudSync.badge}</Badge>
                  <Radio className="size-4 text-muted-foreground" />
                </div>
                <CardTitle>{t.sources.cloudSync.title}</CardTitle>
                <CardDescription>{t.sources.cloudSync.description}</CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-2.5">
                {t.sources.cloudSync.points.map((point) => (
                  <div key={point} className="flex items-start gap-2.5 text-sm text-foreground">
                    <Check className="mt-0.5 size-4 shrink-0 text-brand" />
                    <span className="text-muted-foreground">{point}</span>
                  </div>
                ))}
              </CardContent>
            </Card>
          </div>
        </div>
      </div>
    </section>
  );
}
