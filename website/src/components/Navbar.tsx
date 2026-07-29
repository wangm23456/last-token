import { RELEASES_URL, REPO_URL } from '../lib/utils'
import { useLocale } from '../i18n/LocaleContext'
import type { Locale } from '../i18n/copy'

export function Navbar() {
  const { locale, setLocale, t } = useLocale()

  const links = [
    { href: '#features', label: t.nav.features },
    { href: '#providers', label: t.nav.providers },
    { href: '#download', label: t.nav.download },
  ]

  return (
    <header className="sticky top-0 z-50 border-b border-border/60 bg-background/70 backdrop-blur-xl">
      <div className="mx-auto flex h-14 max-w-6xl items-center justify-between px-4 sm:px-6">
        <a href="#top" className="group flex items-center gap-2.5">
          <img
            src="/last-token.png"
            alt="Last Token"
            className="h-7 w-7 rounded-md object-contain transition-transform duration-200 group-active:scale-95"
          />
          <span className="text-sm font-semibold tracking-tight">Last Token</span>
        </a>

        <nav className="hidden items-center gap-1 sm:flex">
          {links.map((link) => (
            <a
              key={link.href}
              href={link.href}
              className="rounded-md px-3 py-1.5 text-sm text-muted-foreground transition-colors duration-150 hover:bg-muted hover:text-foreground"
            >
              {link.label}
            </a>
          ))}
        </nav>

        <div className="flex items-center gap-2">
          <LanguageSwitch locale={locale} setLocale={setLocale} />
          <a
            href={REPO_URL}
            target="_blank"
            rel="noreferrer"
            className="hidden rounded-md px-3 py-1.5 text-sm text-muted-foreground transition-colors duration-150 hover:text-foreground sm:inline-flex"
          >
            {t.nav.github}
          </a>
          <a
            href={RELEASES_URL}
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center rounded-lg bg-primary px-3.5 py-1.5 text-sm font-medium text-primary-foreground transition-transform duration-150 hover:opacity-90 active:scale-[0.97]"
          >
            {t.nav.downloadCta}
          </a>
        </div>
      </div>
    </header>
  )
}

function LanguageSwitch({
  locale,
  setLocale,
}: {
  locale: Locale
  setLocale: (locale: Locale) => void
}) {
  return (
    <div className="inline-flex items-center rounded-lg border border-border/70 bg-card/50 p-0.5 text-xs">
      <button
        type="button"
        onClick={() => setLocale('en')}
        className={`rounded-md px-2 py-1 transition-colors ${
          locale === 'en'
            ? 'bg-primary text-primary-foreground'
            : 'text-muted-foreground hover:text-foreground'
        }`}
        aria-pressed={locale === 'en'}
      >
        EN
      </button>
      <button
        type="button"
        onClick={() => setLocale('zh')}
        className={`rounded-md px-2 py-1 transition-colors ${
          locale === 'zh'
            ? 'bg-primary text-primary-foreground'
            : 'text-muted-foreground hover:text-foreground'
        }`}
        aria-pressed={locale === 'zh'}
      >
        中文
      </button>
    </div>
  )
}
