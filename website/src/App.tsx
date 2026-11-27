import { useEffect, useState } from "react";
import { ClientsSection } from "@/components/ClientsSection";
import { DownloadSection } from "@/components/DownloadSection";
import { EngineSection } from "@/components/EngineSection";
import { Footer } from "@/components/Footer";
import { HeroSection } from "@/components/HeroSection";
import { Navbar } from "@/components/Navbar";
import { PrivacySection } from "@/components/PrivacySection";
import { SourcesSection } from "@/components/SourcesSection";
import { TooltipProvider } from "@/components/ui/tooltip";
import {
  getInitialLocale,
  nextLocale,
  SITE_STRINGS,
  type SiteLocale,
} from "@/i18n";
import { useLatestRelease } from "@/lib/release";

function getInitialTheme(): "dark" | "light" {
  if (typeof document !== "undefined" && document.documentElement.classList.contains("light")) {
    return "light";
  }
  return "dark";
}

export function App() {
  const [locale, setLocale] = useState<SiteLocale>(() => getInitialLocale());
  const [theme, setTheme] = useState<"dark" | "light">(() => getInitialTheme());
  const release = useLatestRelease();
  const t = SITE_STRINGS[locale];

  useEffect(() => {
    document.documentElement.lang = locale;
    try {
      localStorage.setItem("sonata-site-lang", locale);
    } catch {
      // Ignore storage access errors.
    }
  }, [locale]);

  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove("dark", "light");
    root.classList.add(theme);
    const meta = document.querySelector('meta[name="theme-color"]');
    if (meta) {
      meta.setAttribute("content", theme === "dark" ? "#09090b" : "#f5f5f7");
    }
    try {
      localStorage.setItem("sonata-site-theme", theme);
    } catch {
      // Ignore storage access errors.
    }
  }, [theme]);

  return (
    <TooltipProvider>
      <div className="min-h-screen bg-background text-foreground">
        <Navbar
          t={t}
          locale={locale}
          onCycleLocale={() => setLocale((prev) => nextLocale(prev))}
          theme={theme}
          onToggleTheme={() => setTheme((prev) => (prev === "dark" ? "light" : "dark"))}
          version={release.version}
        />

        <main id="main">
          <HeroSection t={t} version={release.version} />
          <ClientsSection t={t} />
          <EngineSection t={t} />
          <SourcesSection t={t} />
          <DownloadSection t={t} release={release} />
          <PrivacySection t={t} />
        </main>

        <Footer t={t} />
      </div>
    </TooltipProvider>
  );
}

export default App;
