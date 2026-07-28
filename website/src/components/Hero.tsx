import { Apple, Download, Monitor, Terminal } from 'lucide-react'
import { RELEASES_URL, REPO_URL } from '../lib/utils'

function GithubIcon({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="currentColor"
      aria-hidden="true"
      className={className}
    >
      <path d="M12 2C6.477 2 2 6.586 2 12.253c0 4.537 2.865 8.382 6.839 9.743.5.094.682-.222.682-.493 0-.243-.009-.888-.014-1.743-2.782.619-3.37-1.372-3.37-1.372-.454-1.181-1.11-1.496-1.11-1.496-.908-.637.069-.624.069-.624 1.003.072 1.531 1.056 1.531 1.056.892 1.566 2.341 1.114 2.91.852.091-.662.35-1.114.636-1.371-2.22-.259-4.555-1.14-4.555-5.076 0-1.121.39-2.038 1.029-2.756-.103-.259-.446-1.302.098-2.714 0 0 .84-.276 2.75 1.052A9.35 9.35 0 0 1 12 6.844a9.35 9.35 0 0 1 2.504.346c1.909-1.328 2.748-1.052 2.748-1.052.546 1.412.203 2.455.1 2.714.64.718 1.028 1.635 1.028 2.756 0 3.947-2.338 4.814-4.566 5.068.359.317.679.943.679 1.901 0 1.371-.013 2.477-.013 2.814 0 .273.18.592.688.491C19.138 20.63 22 16.787 22 12.253 22 6.586 17.523 2 12 2Z" />
    </svg>
  )
}

export function Hero() {
  return (
    <section id="top" className="relative overflow-hidden pt-16 pb-20 sm:pt-24 sm:pb-28">
      <div className="pointer-events-none absolute inset-0 bg-[linear-gradient(to_right,oklch(0.24_0.01_260_/_0.35)_1px,transparent_1px),linear-gradient(to_bottom,oklch(0.24_0.01_260_/_0.35)_1px,transparent_1px)] bg-[size:48px_48px] [mask-image:radial-gradient(ellipse_60%_50%_at_50%_0%,black,transparent)]" />

      <div className="relative mx-auto grid max-w-6xl items-center gap-12 px-4 sm:px-6 lg:grid-cols-[1.05fr_0.95fr]">
        <div className="space-y-7">
          <div className="inline-flex items-center gap-2 rounded-full border border-border/80 bg-card/60 px-3 py-1 text-xs text-muted-foreground backdrop-blur">
            <span className="h-1.5 w-1.5 animate-pulse-soft rounded-full bg-status-safe" />
            本地优先 · 隐私安全 · 开源 MIT
          </div>

          <div className="space-y-4">
            <h1 className="text-balance text-4xl font-semibold tracking-tight sm:text-5xl lg:text-[3.25rem] lg:leading-[1.1]">
              API 额度耗尽，
              <br />
              <span className="bg-gradient-to-r from-white via-white to-status-safe bg-clip-text text-transparent">
                不该是惊喜
              </span>
            </h1>
            <p className="max-w-xl text-base leading-relaxed text-muted-foreground sm:text-lg">
              跨平台 LLM / AI API 额度实时监测与耗尽预警。自动扫描本地 CLI
              凭证，计算消耗速率，在额度见底前发出提醒。
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-3">
            <a
              href="#download"
              className="inline-flex items-center gap-2 rounded-xl bg-primary px-5 py-2.5 text-sm font-semibold text-primary-foreground transition-transform duration-150 hover:opacity-90 active:scale-[0.97]"
            >
              <Download className="h-4 w-4" />
              免费下载
            </a>
            <a
              href={REPO_URL}
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center gap-2 rounded-xl border border-border bg-card/50 px-5 py-2.5 text-sm font-medium text-foreground transition-colors duration-150 hover:bg-muted active:scale-[0.97]"
            >
              <GithubIcon className="h-4 w-4" />
              查看源码
            </a>
          </div>

          <div className="flex flex-wrap gap-4 pt-1 text-xs text-muted-foreground">
            <span className="inline-flex items-center gap-1.5">
              <Apple className="h-3.5 w-3.5" /> macOS
            </span>
            <span className="inline-flex items-center gap-1.5">
              <Monitor className="h-3.5 w-3.5" /> Windows
            </span>
            <span className="inline-flex items-center gap-1.5">
              <Terminal className="h-3.5 w-3.5" /> Linux
            </span>
          </div>
        </div>

        <div className="relative animate-float">
          <div className="absolute -inset-6 rounded-[2rem] bg-status-safe/10 blur-3xl" />
          <AppPreview />
        </div>
      </div>

      <p className="sr-only">
        Download from <a href={RELEASES_URL}>GitHub Releases</a>
      </p>
    </section>
  )
}

function AppPreview() {
  const providers = [
    { name: 'Claude', used: 72, color: 'bg-status-warning', eta: '约 4.2h' },
    { name: 'Copilot', used: 38, color: 'bg-status-safe', eta: '约 18h' },
    { name: 'Gemini', used: 91, color: 'bg-status-danger', eta: '约 0.8h' },
    { name: 'Kimi', used: 54, color: 'bg-status-safe', eta: '约 9h' },
  ]

  return (
    <div className="relative rounded-2xl border border-border/80 bg-card/80 p-1 shadow-2xl shadow-black/40 backdrop-blur">
      <div className="overflow-hidden rounded-[0.9rem] border border-border/50 bg-background">
        <div className="flex items-center justify-between border-b border-border/60 px-4 py-3">
          <div className="flex items-center gap-2">
            <img src="/last-token.png" alt="" className="h-5 w-5 rounded object-contain" />
            <span className="text-sm font-semibold tracking-tight">Last Token</span>
          </div>
          <div className="flex items-center gap-1.5">
            <span className="h-1.5 w-1.5 animate-pulse-soft rounded-full bg-status-safe" />
            <span className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
              运行中
            </span>
          </div>
        </div>

        <div className="space-y-3 p-4">
          <div className="grid grid-cols-3 gap-2">
            {[
              { label: '监测中', value: '8' },
              { label: '预警', value: '1' },
              { label: '重置周期', value: '5h' },
            ].map((stat) => (
              <div
                key={stat.label}
                className="rounded-lg border border-border/60 bg-card/70 px-3 py-2.5"
              >
                <div className="text-[10px] text-muted-foreground">{stat.label}</div>
                <div className="mt-0.5 text-lg font-semibold tracking-tight">{stat.value}</div>
              </div>
            ))}
          </div>

          <div className="space-y-2.5 rounded-xl border border-border/60 bg-card/40 p-3">
            <div className="flex items-center justify-between text-xs">
              <span className="font-medium">额度概览</span>
              <span className="text-muted-foreground">实时消耗 · %/小时</span>
            </div>
            {providers.map((p, i) => (
              <div key={p.name} className="space-y-1.5">
                <div className="flex items-center justify-between text-xs">
                  <span className="text-muted-foreground">{p.name}</span>
                  <span className="tabular-nums">
                    {p.used}% · 预计耗尽 {p.eta}
                  </span>
                </div>
                <div className="h-1.5 overflow-hidden rounded-full bg-muted">
                  <div
                    className={`h-full rounded-full ${p.color} animate-bar-fill`}
                    style={{ width: `${p.used}%`, animationDelay: `${i * 120}ms` }}
                  />
                </div>
              </div>
            ))}
          </div>

          <div className="rounded-lg border border-status-danger/30 bg-status-danger/10 px-3 py-2 text-xs text-status-danger">
            Gemini 额度将在 1 小时内耗尽 — 建议切换备用提供商
          </div>
        </div>
      </div>
    </div>
  )
}
