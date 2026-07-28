const providers = [
  { name: 'Claude', detail: 'CLI / Code' },
  { name: 'GitHub Copilot', detail: '配额监测' },
  { name: 'Gemini', detail: 'CLI OAuth' },
  { name: 'Kimi', detail: 'For Coding' },
  { name: 'MiniMax', detail: '编程套餐' },
  { name: '智谱 GLM', detail: '个人 / 团队' },
  { name: '火山方舟', detail: 'Ark' },
  { name: 'ZenMux', detail: '代理路由' },
]

export function Providers() {
  return (
    <section id="providers" className="border-t border-border/50 py-20 sm:py-24">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <div className="grid items-start gap-10 lg:grid-cols-[0.9fr_1.1fr]">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.2em] text-status-safe">
              Providers
            </p>
            <h2 className="mt-3 text-3xl font-semibold tracking-tight sm:text-4xl">
              一次看清所有额度
            </h2>
            <p className="mt-3 max-w-md text-muted-foreground">
              自动识别本地官方 CLI 授权与环境变量（如
              ANTHROPIC_API_KEY、MOONSHOT_API_KEY），统一汇入监测面板。
            </p>
          </div>

          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            {providers.map((provider) => (
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
