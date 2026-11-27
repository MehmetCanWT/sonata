import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { SiteStrings } from "@/i18n";

interface EngineSectionProps {
  t: SiteStrings;
}

export function EngineSection({ t }: EngineSectionProps) {
  const nodes = t.engine.diagramNodes;

  return (
    <section id="engine" className="border-t border-border py-20 lg:py-24">
      <div className="mx-auto flex w-full max-w-6xl flex-col gap-10 px-6">
        <div className="flex max-w-2xl flex-col gap-3">
          <h2 className="text-3xl font-semibold tracking-tight text-foreground sm:text-4xl">
            {t.engine.heading}
          </h2>
          <p className="text-base leading-relaxed text-muted-foreground">
            {t.engine.lead}
          </p>
        </div>

        <div className="grid items-start gap-8 lg:grid-cols-12">
          <Card className="min-w-0 lg:col-span-5">
            <CardHeader>
              <CardTitle>{t.engine.diagramTitle}</CardTitle>
              <CardDescription>{t.engine.diagramSubtitle}</CardDescription>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
              <div className="overflow-hidden rounded-lg border border-border bg-background p-4">
                <svg
                  viewBox="0 0 420 272"
                  className="h-auto w-full"
                  role="img"
                  aria-label={t.engine.diagramTitle}
                >
                  <line
                    x1="100"
                    y1="56"
                    x2="132"
                    y2="104"
                    stroke="currentColor"
                    strokeOpacity="0.32"
                    strokeWidth="1.5"
                  />
                  <line
                    x1="312"
                    y1="56"
                    x2="280"
                    y2="104"
                    stroke="currentColor"
                    strokeOpacity="0.32"
                    strokeWidth="1.5"
                  />
                  <line
                    x1="132"
                    y1="150"
                    x2="178"
                    y2="202"
                    stroke="var(--brand)"
                    strokeWidth="1.75"
                  />
                  <line
                    x1="280"
                    y1="150"
                    x2="242"
                    y2="202"
                    stroke="var(--brand)"
                    strokeWidth="1.75"
                  />

                  <rect
                    x="14"
                    y="14"
                    width="176"
                    height="42"
                    rx="8"
                    fill="var(--secondary)"
                    stroke="currentColor"
                    strokeOpacity="0.18"
                  />
                  <text
                    x="102"
                    y="39"
                    textAnchor="middle"
                    fill="currentColor"
                    fontSize="11.5"
                    fontWeight="500"
                  >
                    sonata-desktop · sonata-tui
                  </text>

                  <rect
                    x="220"
                    y="14"
                    width="186"
                    height="42"
                    rx="8"
                    fill="var(--secondary)"
                    stroke="currentColor"
                    strokeOpacity="0.18"
                  />
                  <text
                    x="313"
                    y="39"
                    textAnchor="middle"
                    fill="currentColor"
                    fontSize="11.5"
                    fontWeight="500"
                  >
                    android/ (Compose + Media3)
                  </text>

                  <rect
                    x="36"
                    y="104"
                    width="170"
                    height="46"
                    rx="8"
                    fill="var(--card)"
                    stroke="currentColor"
                    strokeOpacity="0.24"
                  />
                  <text
                    x="121"
                    y="131"
                    textAnchor="middle"
                    fill="currentColor"
                    fontSize="11.5"
                    fontWeight="500"
                  >
                    sonata-runtime
                  </text>

                  <rect
                    x="224"
                    y="104"
                    width="168"
                    height="46"
                    rx="8"
                    fill="var(--card)"
                    stroke="currentColor"
                    strokeOpacity="0.24"
                  />
                  <text
                    x="308"
                    y="131"
                    textAnchor="middle"
                    fill="currentColor"
                    fontSize="11.5"
                    fontWeight="500"
                  >
                    sonata-android (UniFFI)
                  </text>

                  <rect
                    x="44"
                    y="202"
                    width="332"
                    height="54"
                    rx="10"
                    fill="var(--secondary)"
                    stroke="var(--brand)"
                    strokeWidth="1.5"
                  />
                  <text
                    x="210"
                    y="226"
                    textAnchor="middle"
                    fill="currentColor"
                    fontSize="13"
                    fontWeight="600"
                  >
                    sonata-core
                  </text>
                  <text
                    x="210"
                    y="243"
                    textAnchor="middle"
                    fill="var(--muted-foreground)"
                    fontSize="10.5"
                  >
                    Providers · SQLite WAL · Lyrics · Cipher · Player
                  </text>
                </svg>
              </div>

              <ul className="flex flex-col gap-2 text-xs text-muted-foreground">
                <li className="flex items-center justify-between gap-2 border-b border-border pb-2">
                  <span className="font-mono text-foreground">{nodes.core}</span>
                </li>
                <li className="flex items-center justify-between gap-2 border-b border-border pb-2">
                  <span className="font-mono text-foreground">{nodes.runtime}</span>
                </li>
                <li className="flex items-center justify-between gap-2 border-b border-border pb-2">
                  <span className="font-mono text-foreground">{nodes.androidFfi}</span>
                </li>
                <li className="flex items-center justify-between gap-2">
                  <span className="font-mono text-foreground">
                    {nodes.desktop} · {nodes.tui} · {nodes.androidApp}
                  </span>
                </li>
              </ul>
            </CardContent>
          </Card>

          <div className="flex min-w-0 flex-col gap-6 lg:col-span-7">
            {t.engine.blocks.map((block) => (
              <Card key={block.title}>
                <CardHeader>
                  <CardTitle>{block.title}</CardTitle>
                  <CardDescription>{block.subtitle}</CardDescription>
                </CardHeader>
                <CardContent className="grid gap-4 sm:grid-cols-2">
                  {block.items.map((item) => (
                    <div
                      key={item.label}
                      className="flex flex-col gap-1.5 rounded-lg border border-border bg-background/60 p-4"
                    >
                      <span className="text-sm font-medium text-foreground">
                        {item.label}
                      </span>
                      <span className="text-xs leading-relaxed text-muted-foreground">
                        {item.detail}
                      </span>
                    </div>
                  ))}
                </CardContent>
              </Card>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}
