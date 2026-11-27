export type SiteLocale = "en" | "tr" | "ru";

export type PlatformId = "linux" | "android" | "macos" | "windows" | "source";

export interface SiteStrings {
  skipToContent: string;
  brandName: string;
  nav: {
    clients: string;
    engine: string;
    sources: string;
    download: string;
    privacy: string;
    github: string;
    openMenu: string;
    closeMenu: string;
    menuTitle: string;
    menuDescription: string;
    toggleTheme: string;
    darkLabel: string;
    lightLabel: string;
    switchLanguage: string;
  };
  hero: {
    eyebrow: string;
    headline: string;
    lead: string;
    primaryCta: string;
    secondaryCta: string;
    imageAlt: string;
    metrics: Array<{ value: string; label: string }>;
  };
  clients: {
    heading: string;
    lead: string;
    tabs: {
      desktop: string;
      tui: string;
      android: string;
    };
    desktopPanel: {
      title: string;
      summary: string;
      imageAlt: string;
      specs: Array<{ label: string; value: string }>;
    };
    tuiPanel: {
      title: string;
      summary: string;
      imageAlt: string;
      specs: Array<{ label: string; value: string }>;
    };
    androidPanel: {
      title: string;
      summary: string;
      architectureNote: string;
      specs: Array<{ label: string; value: string }>;
      highlights: Array<{ title: string; detail: string }>;
    };
    matrixTitle: string;
    matrixHeaders: [string, string, string, string];
    matrixRows: Array<[string, string, string, string]>;
  };
  engine: {
    heading: string;
    lead: string;
    diagramTitle: string;
    diagramSubtitle: string;
    diagramNodes: {
      tui: string;
      desktop: string;
      runtime: string;
      androidApp: string;
      androidFfi: string;
      core: string;
    };
    blocks: Array<{
      title: string;
      subtitle: string;
      items: Array<{ label: string; detail: string }>;
    }>;
  };
  sources: {
    heading: string;
    lead: string;
    tableHeaders: [string, string, string];
    protocols: Array<{ name: string; transport: string; features: string }>;
    personalMode: {
      badge: string;
      title: string;
      description: string;
      points: string[];
    };
    cloudSync: {
      badge: string;
      title: string;
      description: string;
      points: string[];
    };
  };
  download: {
    heading: string;
    lead: string;
    tabs: Record<PlatformId, string>;
    copyLabel: string;
    copiedLabel: string;
    linux: {
      oneLinersTitle: string;
      desktopInstallLabel: string;
      tuiInstallLabel: string;
      packagesTitle: string;
      packagesNote: string;
      tuiFlagHint: string;
      distrosLabel: string;
      distrosItems: Array<{ label: string; detail: string; href: string }>;
    };
    android: {
      title: string;
      description: string;
      downloadApkCta: string;
      obtainiumTitle: string;
      obtainiumRepoLabel: string;
      obtainiumFilterLabel: string;
      updaterNote: string;
    };
    macos: {
      title: string;
      description: string;
      dmgLabel: string;
      tarballLabel: string;
      requirementsNote: string;
    };
    windows: {
      title: string;
      description: string;
      setupLabel: string;
      msiLabel: string;
      portableLabel: string;
      requirementsNote: string;
    };
    source: {
      title: string;
      description: string;
      rustLabel: string;
      androidLabel: string;
      verifyLabel: string;
    };
    checksumsLabel: string;
    allReleasesLabel: string;
  };
  privacy: {
    heading: string;
    lead: string;
    facts: Array<{ title: string; detail: string }>;
    readPolicyCta: string;
    faqTitle: string;
    faqs: Array<{ question: string; answer: string }>;
  };
  footer: {
    disclaimer: string;
    licenseLine: string;
    groups: Array<{
      title: string;
      links: Array<{ label: string; href: string }>;
    }>;
  };
}
