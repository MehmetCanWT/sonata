import { Separator } from "@/components/ui/separator";
import type { SiteStrings } from "@/i18n";

interface FooterProps {
  t: SiteStrings;
}

export function Footer({ t }: FooterProps) {
  return (
    <footer className="border-t border-border bg-background py-14">
      <div className="mx-auto flex w-full max-w-6xl flex-col gap-10 px-6">
        <div className="grid gap-8 md:grid-cols-12">
          <div className="flex flex-col gap-3 md:col-span-6">
            <div className="flex items-center gap-2.5">
              <img
                src="./media/sonata-logo.svg"
                alt=""
                width={24}
                height={24}
                loading="lazy"
                className="size-6 rounded-md"
              />
              <span className="text-base font-semibold text-foreground">
                {t.brandName}
              </span>
            </div>
            <p className="max-w-md text-xs leading-relaxed text-muted-foreground">
              {t.footer.disclaimer}
            </p>
          </div>

          <div className="grid gap-8 sm:grid-cols-2 md:col-span-6">
            {t.footer.groups.map((group) => (
              <div key={group.title} className="flex flex-col gap-3">
                <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  {group.title}
                </h3>
                <ul className="flex flex-col gap-1">
                  {group.links.map((link) => (
                    <li key={link.label}>
                      <a
                        href={link.href}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="inline-flex min-h-11 items-center text-sm text-muted-foreground transition-colors hover:text-foreground"
                      >
                        {link.label}
                      </a>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        </div>

        <Separator />

        <p className="text-xs text-muted-foreground">{t.footer.licenseLine}</p>
      </div>
    </footer>
  );
}
