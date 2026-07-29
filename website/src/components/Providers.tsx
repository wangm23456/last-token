import { useLocale } from '../i18n/LocaleContext'

export function Providers() {
  const { t } = useLocale()

  return (
    <section id="providers" className="border-t border-border/50 py-20 sm:py-24">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <div className="grid items-start gap-10 lg:grid-cols-[0.9fr_1.1fr]">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.2em] text-status-safe">
              {t.providers.eyebrow}
            </p>
            <h2 className="mt-3 text-3xl font-semibold tracking-tight sm:text-4xl">
              {t.providers.title}
            </h2>
            <p className="mt-3 max-w-md text-muted-foreground">{t.providers.subtitle}</p>
          </div>

          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            {t.providers.items.map((provider) => (
              <div
                key={provider.name}
                className="rounded-xl border border-border/70 bg-card/50 px-4 py-4 transition-colors duration-150 hover:bg-card"
              >
                <div className="text-sm font-semibold tracking-tight">{provider.name}</div>
                <div className="mt-1 text-xs text-muted-foreground">{provider.detail}</div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </section>
  )
}
