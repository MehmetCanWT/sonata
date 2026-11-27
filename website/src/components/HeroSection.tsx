import { ArrowRight, Download } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import type { SiteStrings } from "@/i18n";

interface HeroSectionProps {
  t: SiteStrings;
  version: string;
}

export function HeroSection({ t, version }: HeroSectionProps) {
  return (
    <section
      id="top"
      className="mx-auto flex min-h-[calc(100svh-4rem)] w-full max-w-6xl items-center px-6 pt-24 pb-12 lg:pt-24 lg:pb-14"
    >
      <div className="hero-enter grid w-full items-center gap-10 lg:grid-cols-12 lg:gap-10">
        <div className="flex min-w-0 flex-col gap-6 lg:col-span-5">
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant="brand">v{version}</Badge>
            <span className="text-xs font-medium text-muted-foreground">
              {t.hero.eyebrow}
            </span>
          </div>

          <div className="flex flex-col gap-4">
            <h1 className="text-3xl font-semibold tracking-tight text-foreground sm:text-4xl lg:text-[2.75rem] lg:leading-[1.08]">
              {t.hero.headline}
            </h1>
            <p className="text-base leading-relaxed text-muted-foreground">
              {t.hero.lead}
            </p>
          </div>

          <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
            <Button size="lg" asChild>
              <a href="#download">
                <Download data-icon="inline-start" />
                {t.hero.primaryCta}
              </a>
            </Button>
            <Button variant="outline" size="lg" asChild>
              <a
                href="https://github.com/MehmetCanWT/sonata"
                target="_blank"
                rel="noopener noreferrer"
              >
                {t.hero.secondaryCta}
                <ArrowRight data-icon="inline-end" />
              </a>
            </Button>
          </div>

          <Separator />

          <dl className="grid grid-cols-2 gap-x-6 gap-y-4 sm:grid-cols-4">
            {t.hero.metrics.map((metric) => (
              <div key={metric.label} className="flex flex-col gap-1">
                <dt className="text-xs text-muted-foreground">{metric.label}</dt>
                <dd className="order-first text-xl font-semibold tracking-tight text-foreground tabular-nums">
                  {metric.value}
                </dd>
              </div>
            ))}
          </dl>
        </div>

        <div className="min-w-0 lg:col-span-7">
          <div className="tactile-card overflow-hidden rounded-2xl border border-border bg-card">
            <img
              src="./media/sonata-android.webp"
              alt={t.hero.imageAlt}
              width={1200}
              height={675}
              fetchPriority="high"
              className="art-zoom block aspect-video w-full object-cover"
            />
          </div>
        </div>
      </div>
    </section>
  );
}
