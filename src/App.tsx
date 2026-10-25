import { createContext, useCallback, useContext, useEffect, useState } from "react";
import { api, errorText, openExternal } from "./api/client";
import type { CatalogView } from "./api/types";
import { useI18n, type Key, type LangSetting } from "./i18n";
import { NAMES } from "./i18n/lang";
import { localizeBackend } from "./i18n/backend";
import { hostOf } from "./lib/format";
import { HomePage } from "./pages/HomePage";
import { CatalogPage } from "./pages/CatalogPage";
import { VerifyPage } from "./pages/VerifyPage";
import { BuildPage } from "./pages/BuildPage";
import { WizardPage } from "./pages/WizardPage";
import { NotebookPage } from "./pages/NotebookPage";
import { CommunityPage } from "./pages/CommunityPage";
import { SourcesPage } from "./pages/SourcesPage";
import { AboutPage } from "./pages/AboutPage";
import "./App.css";

export type Page = "home" | "catalog" | "verify" | "build" | "wizard" | "notebook" | "community" | "sources" | "about";

interface AppContextValue {
  view: CatalogView | null;
  setView: (v: CatalogView) => void;
  refreshing: boolean;
  refresh: () => Promise<void>;
  refreshError: string | null;
  navigate: (page: Page) => void;
  /** Ask before opening any external link; shows the destination domain. */
  requestOpen: (url: string, what: string) => void;
  verifyTarget: string | null;
  verifyAgainst: (key: string | null) => void;
  /** Catalog entry the Build page should start with (set by the recovery wizard). */
  buildTarget: string | null;
  buildFor: (key: string | null) => void;
}

const AppContext = createContext<AppContextValue | null>(null);

export function useApp(): AppContextValue {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error("useApp outside provider");
  return ctx;
}

const NAV: { page: Page; label: Key }[] = [
  { page: "home", label: "nav.home" },
  { page: "wizard", label: "nav.wizard" },
  { page: "catalog", label: "nav.catalog" },
  { page: "verify", label: "nav.verify" },
  { page: "build", label: "nav.build" },
  { page: "notebook", label: "nav.notebook" },
  { page: "community", label: "nav.community" },
  { page: "sources", label: "nav.sources" },
  { page: "about", label: "nav.about" },
];


export default function App() {
  const { t, lang, setting, setSetting, languages } = useI18n();
  const [page, setPage] = useState<Page>("home");
  const [view, setView] = useState<CatalogView | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const [pendingOpen, setPendingOpen] = useState<{ url: string; what: string } | null>(null);
  const [verifyTarget, setVerifyTarget] = useState<string | null>(null);
  const [buildTarget, setBuildTarget] = useState<string | null>(null);

  useEffect(() => {
    api.getCatalog().then(setView, (e) => setLoadError(errorText(e)));
  }, []);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    setRefreshError(null);
    try {
      setView(await api.refreshCatalog());
    } catch (e) {
      setRefreshError(errorText(e));
    } finally {
      setRefreshing(false);
    }
  }, []);

  const ctx: AppContextValue = {
    view,
    setView,
    refreshing,
    refresh,
    refreshError,
    navigate: setPage,
    requestOpen: (url, what) => setPendingOpen({ url, what }),
    verifyTarget,
    buildTarget,
    buildFor: setBuildTarget,
    verifyAgainst: (key) => {
      setVerifyTarget(key);
      setPage("verify");
    },
  };

  return (
    <AppContext.Provider value={ctx}>
      <div className="shell">
        <nav className="sidebar" aria-label={t("nav.main")}>
          <div className="brand">
            <span className="brand-mark" aria-hidden="true" />
            <div>
              <div className="brand-name">VoidRecoverPS5</div>
              <div className="brand-sub">{t("app.brandSub")}</div>
            </div>
          </div>
          <ul className="nav">
            {NAV.map((n) => (
              <li key={n.page}>
                <button
                  className={page === n.page ? "nav-item active" : "nav-item"}
                  aria-current={page === n.page ? "page" : undefined}
                  onClick={() => setPage(n.page)}
                >
                  {t(n.label)}
                </button>
              </li>
            ))}
          </ul>
          <label className="lang-switch">
            <span className="lang-label">{t("lang.label")}</span>
            <select value={setting} onChange={(e) => setSetting(e.target.value as LangSetting)}>
              <option value="auto">{t("lang.auto")}</option>
              {languages.map((l) => (
                <option key={l} value={l} lang={l}>
                  {NAMES[l]}
                </option>
              ))}
            </select>
          </label>
          <div className="sidebar-foot">
            {t("app.footer", { version: view?.app_version ?? "…", catalog: view?.catalog_version ?? "…" })}
          </div>
        </nav>

        <main className="content" id="main">
          {loadError && <div className="banner danger">{t("app.loadError", { error: localizeBackend(loadError, t, lang) })}</div>}
          {page === "home" && <HomePage />}
          {page === "catalog" && <CatalogPage />}
          {page === "verify" && <VerifyPage />}
          {page === "build" && <BuildPage />}
          {page === "wizard" && <WizardPage />}
          {page === "notebook" && <NotebookPage />}
          {page === "community" && <CommunityPage />}
          {page === "sources" && <SourcesPage />}
          {page === "about" && <AboutPage />}
        </main>
      </div>

      {pendingOpen && (
        <OpenConfirm
          url={pendingOpen.url}
          what={pendingOpen.what}
          onClose={() => setPendingOpen(null)}
        />
      )}
    </AppContext.Provider>
  );
}

function OpenConfirm({ url, what, onClose }: { url: string; what: string; onClose: () => void }) {
  const { t, tx } = useI18n();
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  return (
    <div className="modal-backdrop" role="presentation" onClick={onClose}>
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="open-title"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 id="open-title">{t("open.title")}</h2>
        <p>{tx("open.body", { what, host: hostOf(url) })}</p>
        <p className="mono small break">{url}</p>
        {error && <div className="banner danger">{error}</div>}
        <div className="modal-actions">
          <button className="btn" onClick={onClose} autoFocus>
            {t("open.cancel")}
          </button>
          <button
            className="btn primary"
            onClick={() =>
              openExternal(url).then(onClose, (e) => setError(t("open.failed", { error: errorText(e) })))
            }
          >
            {t("open.open", { host: hostOf(url) })}
          </button>
        </div>
      </div>
    </div>
  );
}
