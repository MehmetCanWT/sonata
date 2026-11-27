import { ArrowUpRight, ShieldCheck } from "lucide-react";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { SiteStrings } from "@/i18n";

interface PrivacySectionProps {
  t: SiteStrings;
}

export function PrivacySection({ t }: PrivacySectionProps) {
  return (
    <section
      id="privacy"
      className="border-t border-border bg-card/35 py-20 lg:py-24"
    >
      <div className="mx-auto flex w-full max-w-6xl flex-col gap-10 px-6">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div className="flex max-w-2xl flex-col gap-3">
            <h2 className="text-3xl font-semibold tracking-tight text-foreground sm:text-4xl">
              {t.privacy.heading}
            </h2>
            <p className="text-base leading-relaxed text-muted-foreground">
              {t.privacy.lead}
            </p>
          </div>
          <Button variant="outline" asChild>
            <a
              href="https://github.com/MehmetCanWT/sonata/blob/main/PRIVACY.md"
              target="_blank"
              rel="noopener noreferrer"
            >
              {t.privacy.readPolicyCta}
              <ArrowUpRight data-icon="inline-end" />
            </a>
          </Button>
        </div>

        <div className="grid items-start gap-8 lg:grid-cols-12">
          <div className="grid min-w-0 gap-4 sm:grid-cols-2 lg:col-span-6">
            {t.privacy.facts.map((fact) => (
              <Card key={fact.title} className="min-w-0">
                <CardHeader className="pb-2">
                  <ShieldCheck className="size-5 text-brand" />
                  <CardTitle className="text-base">{fact.title}</CardTitle>
                </CardHeader>
                <CardContent>
                  <p className="text-xs leading-relaxed text-muted-foreground">
                    {fact.detail}
                  </p>
                </CardContent>
              </Card>
            ))}
          </div>

          <Card className="min-w-0 lg:col-span-6">
            <CardHeader>
              <CardTitle>{t.privacy.faqTitle}</CardTitle>
            </CardHeader>
            <CardContent>
              <Accordion type="single" collapsible defaultValue="item-0">
                {t.privacy.faqs.map((faq, idx) => (
                  <AccordionItem key={faq.question} value={`item-${idx}`}>
                    <AccordionTrigger>{faq.question}</AccordionTrigger>
                    <AccordionContent>{faq.answer}</AccordionContent>
                  </AccordionItem>
                ))}
              </Accordion>
            </CardContent>
          </Card>
        </div>
      </div>
    </section>
  );
}
