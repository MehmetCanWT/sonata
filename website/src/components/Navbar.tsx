import { useState } from "react";
import { ArrowUpRight, Globe, Menu, Moon, Sun } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import {
  Sheet,
  SheetClose,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";
import type { SiteLocale, SiteStrings } from "@/i18n";

interface NavbarProps {
  t: SiteStrings;
  locale: SiteLocale;
  onCycleLocale: () => void;
  theme: "dark" | "light";
  onToggleTheme: () => void;
  version: string;
}

export function Navbar({
  t,
  locale,
  onCycleLocale,
  theme,
  onToggleTheme,
  version,
}: NavbarProps) {
  const [sheetOpen, setSheetOpen] = useState(false);

  const navItems = [
    { href: "#clients", label: t.nav.clients },
    { href: "#engine", label: t.nav.engine },
    { href: "#sources", label: t.nav.sources },
    { href: "#download", label: t.nav.download },
    { href: "#privacy", label: t.nav.privacy },
  ];

  return (
    <header className="fixed inset-x-0 top-0 z-40 border-b border-border bg-background/85 backdrop-blur-md">
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:fixed focus:top-3 focus:left-4 focus:z-50 focus:rounded-md focus:bg-primary focus:px-3 focus:py-2 focus:text-sm focus:font-medium focus:text-primary-foreground"
      >
        {t.skipToContent}
      </a>

      <div className="mx-auto flex h-16 w-full max-w-6xl items-center justify-between gap-4 px-6">
        <div className="flex items-center gap-3">
          <a href="#top" className="inline-flex min-h-11 items-center gap-2.5 text-foreground">
            <img
              src="./media/sonata-logo.svg"
              alt=""
              width={28}
              height={28}
              className="size-7 rounded-md"
            />
            <span className="text-base font-semibold tracking-tight">{t.brandName}</span>
          </a>
          <Badge variant="outline" className="hidden sm:inline-flex">
            v{version}
          </Badge>
        </div>

        <nav aria-label="Primary" className="hidden items-center gap-1 md:flex">
          {navItems.map((item) => (
            <a
              key={item.href}
              href={item.href}
              className="inline-flex min-h-11 items-center rounded-md px-3 text-sm text-muted-foreground transition-colors hover:text-foreground"
            >
              {item.label}
            </a>
          ))}
        </nav>

        <div className="hidden items-center gap-2 md:flex">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={onCycleLocale}
            aria-label={t.nav.switchLanguage}
            className="min-h-11 px-3 font-mono uppercase"
          >
            <Globe data-icon="inline-start" />
            {locale}
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            onClick={onToggleTheme}
            aria-label={t.nav.toggleTheme}
          >
            {theme === "dark" ? <Sun /> : <Moon />}
          </Button>
          <Button variant="outline" size="sm" asChild className="min-h-11 px-3.5">
            <a
              href="https://github.com/MehmetCanWT/sonata"
              target="_blank"
              rel="noopener noreferrer"
            >
              {t.nav.github}
              <ArrowUpRight data-icon="inline-end" />
            </a>
          </Button>
        </div>

        <div className="flex items-center gap-1 md:hidden">
          <Button
            type="button"
            variant="ghost"
            size="icon"
            onClick={onToggleTheme}
            aria-label={t.nav.toggleTheme}
          >
            {theme === "dark" ? <Sun /> : <Moon />}
          </Button>
          <Sheet open={sheetOpen} onOpenChange={setSheetOpen}>
            <SheetTrigger asChild>
              <Button
                type="button"
                variant="outline"
                size="icon"
                aria-label={t.nav.openMenu}
              >
                <Menu />
              </Button>
            </SheetTrigger>
            <SheetContent closeLabel={t.nav.closeMenu}>
              <SheetHeader>
                <SheetTitle>{t.nav.menuTitle}</SheetTitle>
                <SheetDescription>{t.nav.menuDescription}</SheetDescription>
              </SheetHeader>
              <nav aria-label="Mobile" className="flex flex-col gap-1">
                {navItems.map((item) => (
                  <SheetClose asChild key={item.href}>
                    <a
                      href={item.href}
                      className="inline-flex min-h-11 items-center rounded-md px-3 text-sm font-medium text-foreground transition-colors hover:bg-accent"
                    >
                      {item.label}
                    </a>
                  </SheetClose>
                ))}
              </nav>
              <Separator />
              <div className="flex flex-col gap-3">
                <Button
                  type="button"
                  variant="outline"
                  onClick={onCycleLocale}
                  className="justify-between"
                >
                  <span className="inline-flex items-center gap-2">
                    <Globe />
                    {t.nav.switchLanguage}
                  </span>
                  <span className="font-mono uppercase">{locale}</span>
                </Button>
                <Button variant="default" asChild>
                  <a
                    href="https://github.com/MehmetCanWT/sonata"
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    {t.nav.github}
                    <ArrowUpRight data-icon="inline-end" />
                  </a>
                </Button>
              </div>
            </SheetContent>
          </Sheet>
        </div>
      </div>
    </header>
  );
}
