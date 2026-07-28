import {
  Activity,
  FolderSearch,
  Gauge,
  Lock,
  PanelTop,
  Zap,
  type LucideIcon,
} from 'lucide-react'

const features: {
  icon: LucideIcon
  title: string
  description: string
}[] = [
  {
    icon: FolderSearch,
    title: '自动扫描 · 一键导入',
    description:
      '读取本地 CLI 授权配置与 Shell 环境变量，无需手动粘贴密钥，开箱即用。',
  },
  {
    icon: Gauge,
    title: '消耗速率与耗尽预测',
    description:
      '基于时间序列快照计算 %/小时 消耗，并结合重置周期给出精准预警。',
  },
  {
    icon: PanelTop,
    title: '主界面 + 托盘小组件',
    description:
      '完整桌面面板与 macOS 状态栏 / 系统托盘并存，额度一眼可见、不打断心流。',
  },
  {
    icon: Lock,
    title: '隐私与安全第一',
    description:
      'API 凭证仅存本地内存与 SQLite，绝不上传第三方服务器，本地优先架构。',
  },
  {
    icon: Activity,
    title: '多提供商统一视图',
    description:
      'Claude、Copilot、Gemini、Kimi、MiniMax、智谱、火山方舟等配额集中监控。',
  },
  {
    icon: Zap,
    title: '轻量高性能',
    description:
      'Tauri 2 + React + Rust 构建，启动快、占用低，适合长期挂机监测。',
  },
]

export function Features() {
  return (
    <section id="features" className="border-t border-border/50 py-20 sm:py-24">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <div className="mx-auto max-w-2xl text-center">
          <p className="text-xs font-semibold uppercase tracking-[0.2em] text-status-safe">
            Features
          </p>
          <h2 className="mt-3 text-3xl font-semibold tracking-tight sm:text-4xl">
            为高频 API 使用者设计
          </h2>
          <p className="mt-3 text-muted-foreground">
            从发现额度、预测耗尽到及时提醒，Last Token
            帮你把「断额」从突发事故变成可预期事件。
          </p>
        </div>

        <div className="mt-12 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {features.map((feature) => (
            <article
              key={feature.title}
              className="group rounded-2xl border border-border/70 bg-card/40 p-5 transition-colors duration-200 hover:border-border hover:bg-card/70"
            >
              <div className="mb-4 inline-flex h-10 w-10 items-center justify-center rounded-xl border border-border/70 bg-background text-status-safe transition-transform duration-200 group-hover:scale-105">
                <feature.icon className="h-5 w-5" />
              </div>
              <h3 className="text-base font-semibold tracking-tight">{feature.title}</h3>
              <p className="mt-2 text-sm leading-relaxed text-muted-foreground">
                {feature.description}
              </p>
            </article>
          ))}
        </div>
      </div>
    </section>
  )
}
