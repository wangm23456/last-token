import {
  Activity,
  FolderSearch,
  Gauge,
  Lock,
  PanelTop,
  Zap,
  type LucideIcon,
} from 'lucide-react'
import { useLocale } from '../i18n/LocaleContext'

const icons: LucideIcon[] = [FolderSearch, Gauge, PanelTop, Lock, Activity, Zap]

export function Features() {
  const { t } = useLocale()

  return (
    <section id="features" className="border-t border-border/50 py-20 sm:py-24">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <div className="mx-auto max-w-2xl text-center">
          <p className="text-xs font-semibold uppercase tracking-[0.2em] text-status-safe">
            {t.features.eyebrow}
          </p>
          <h2 className="mt-3 text-3xl font-semibold tracking-tight sm:text-4xl">
            {t.features.title}
          </h2>
          <p className="mt-3 text-muted-foreground">{t.features.subtitle}</p>
        </div>

        <div className="mt-12 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {t.features.items.map((feature, index) => {
            const Icon = icons[index] ?? Zap
            return (
              <article
                key={feature.title}
                className="group rounded-2xl border border-border/70 bg-card/40 p-5 transition-colors duration-200 hover:border-border hover:bg-card/70"
              >
                <div className="mb-4 inline-flex h-10 w-10 items-center justify-center rounded-xl border border-border/70 bg-background text-status-safe transition-transform duration-200 group-hover:scale-105">
                  <Icon className="h-5 w-5" />
                </div>
                <h3 className="text-base font-semibold tracking-tight">{feature.title}</h3>
                <p className="mt-2 text-sm leading-relaxed text-muted-foreground">
                  {feature.description}
                </p>
              </article>
            )
          })}
        </div>
      </div>
    </section>
  )
}
