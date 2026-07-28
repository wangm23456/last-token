import { Apple, Download as DownloadIcon, Monitor, Terminal } from 'lucide-react'
import { RELEASES_URL } from '../lib/utils'

const platforms = [
  {
    icon: Apple,
    name: 'macOS',
    formats: '.dmg / .app',
    note: 'Apple Silicon & Intel',
  },
  {
    icon: Monitor,
    name: 'Windows',
    formats: '.exe / .msi',
    note: 'NSIS 安装包',
  },
  {
    icon: Terminal,
    name: 'Linux',
    formats: '.deb / .AppImage',
    note: '主流发行版',
  },
]

const steps = [
  {
    step: '01',
    title: '下载安装包',
    description: '从 GitHub Releases 获取对应系统的安装包并安装。',
  },
  {
    step: '02',
    title: '自动扫描凭证',
    description: '打开应用后一键扫描本地 CLI 配置与环境变量。',
  },
  {
    step: '03',
    title: '盯住额度变化',
    description: '主界面或托盘实时显示消耗速率与耗尽预警。',
  },
]

export function Download() {
  return (
    <section id="download" className="border-t border-border/50 py-20 sm:py-24">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <div className="mx-auto max-w-2xl text-center">
          <p className="text-xs font-semibold uppercase tracking-[0.2em] text-status-safe">
            Download
          </p>
          <h2 className="mt-3 text-3xl font-semibold tracking-tight sm:text-4xl">
            三步开始监测
          </h2>
          <p className="mt-3 text-muted-foreground">
            免费开源，跨平台桌面应用。安装后即可开始保护你的 API 额度。
          </p>
        </div>

        <div className="mt-10 grid gap-4 md:grid-cols-3">
          {steps.map((item) => (
            <div
              key={item.step}
              className="rounded-2xl border border-border/70 bg-card/40 p-5"
            >
              <div className="text-xs font-semibold tracking-[0.18em] text-status-safe">
                {item.step}
              </div>
              <h3 className="mt-3 text-base font-semibold tracking-tight">{item.title}</h3>
              <p className="mt-2 text-sm leading-relaxed text-muted-foreground">
                {item.description}
              </p>
            </div>
          ))}
        </div>

        <div className="mt-8 grid gap-3 sm:grid-cols-3">
          {platforms.map((platform) => (
            <a
              key={platform.name}
              href={RELEASES_URL}
              target="_blank"
              rel="noreferrer"
              className="group flex items-center gap-4 rounded-2xl border border-border/70 bg-card/50 p-5 transition-all duration-200 hover:border-border hover:bg-card active:scale-[0.98]"
            >
              <div className="inline-flex h-11 w-11 items-center justify-center rounded-xl border border-border/70 bg-background text-foreground transition-transform duration-200 group-hover:scale-105">
                <platform.icon className="h-5 w-5" />
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex items-center justify-between gap-2">
                  <span className="font-semibold tracking-tight">{platform.name}</span>
                  <DownloadIcon className="h-4 w-4 text-muted-foreground transition-colors group-hover:text-foreground" />
                </div>
                <div className="mt-0.5 text-xs text-muted-foreground">
                  {platform.formats} · {platform.note}
                </div>
              </div>
            </a>
          ))}
        </div>

        <div className="mt-8 flex justify-center">
          <a
            href={RELEASES_URL}
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-2 rounded-xl bg-primary px-6 py-3 text-sm font-semibold text-primary-foreground transition-transform duration-150 hover:opacity-90 active:scale-[0.97]"
          >
            <DownloadIcon className="h-4 w-4" />
            前往 GitHub Releases
          </a>
        </div>
      </div>
    </section>
  )
}
