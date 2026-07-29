export type Locale = 'en' | 'zh'

export const messages = {
  en: {
    meta: {
      title: 'Last Token — Real-time AI API Quota Monitoring',
      description:
        'Last Token — Cross-platform real-time LLM / AI API quota monitoring and exhaustion alerts. Local-first, privacy-safe, avoid mid-work outages.',
    },
    nav: {
      features: 'Features',
      providers: 'Providers',
      download: 'Download',
      github: 'GitHub',
      downloadCta: 'Download',
    },
    hero: {
      badge: 'Local-first · Privacy-safe · Open-source MIT',
      titleLine1: 'API quota exhaustion',
      titleHighlight: 'shouldn’t be a surprise',
      subtitle:
        'Cross-platform real-time LLM / AI API quota monitoring and exhaustion alerts. Auto-scan local CLI credentials, estimate burn rate, and warn before credits run out.',
      download: 'Download free',
      viewSource: 'View source',
      preview: {
        running: 'Running',
        monitoring: 'Monitoring',
        alerts: 'Alerts',
        resetWindow: 'Reset window',
        overview: 'Quota overview',
        realtime: 'Live burn · %/hour',
        etaPrefix: 'ETA',
        warning: 'Gemini quota will run out within 1 hour — switch to a backup provider',
      },
    },
    features: {
      eyebrow: 'Features',
      title: 'Built for heavy API users',
      subtitle:
        'From discovering quota to predicting exhaustion and alerting in time, Last Token turns sudden outages into expected events.',
      items: [
        {
          title: 'Auto scan · one-click import',
          description:
            'Reads local CLI auth configs and shell environment variables — no manual key pasting required.',
        },
        {
          title: 'Burn rate & exhaustion forecasts',
          description:
            'Time-series snapshots estimate %/hour usage and warn against reset windows accurately.',
        },
        {
          title: 'Dashboard + tray panel',
          description:
            'Full desktop panel plus macOS menu bar / system tray — quotas stay visible without breaking flow.',
        },
        {
          title: 'Privacy first',
          description:
            'API credentials stay in local memory and SQLite only. Nothing is uploaded to third-party servers.',
        },
        {
          title: 'Unified multi-provider view',
          description:
            'Monitor Claude, Copilot, Gemini, Kimi, MiniMax, Zhipu, Volcano Ark, and more in one place.',
        },
        {
          title: 'Lightweight & fast',
          description:
            'Built with Tauri 2 + React + Rust — fast startup, low footprint, ideal for always-on monitoring.',
        },
      ],
    },
    providers: {
      eyebrow: 'Providers',
      title: 'See every quota at once',
      subtitle:
        'Automatically detect official local CLI auth and env vars (like ANTHROPIC_API_KEY, MOONSHOT_API_KEY) and unify them in one dashboard.',
      items: [
        { name: 'Claude', detail: 'CLI / Code' },
        { name: 'GitHub Copilot', detail: 'Quota tracking' },
        { name: 'Gemini', detail: 'CLI OAuth' },
        { name: 'Kimi', detail: 'For Coding' },
        { name: 'MiniMax', detail: 'Coding plans' },
        { name: 'Zhipu GLM', detail: 'Personal / Team' },
        { name: 'Volcano Ark', detail: 'Ark' },
        { name: 'ZenMux', detail: 'Proxy routing' },
      ],
    },
    download: {
      eyebrow: 'Download',
      title: 'Start monitoring in three steps',
      subtitle: 'Free and open source. Install once, then keep your API quotas protected.',
      steps: [
        {
          step: '01',
          title: 'Download the installer',
          description: 'Get the package for your OS from GitHub Releases and install it.',
        },
        {
          step: '02',
          title: 'Auto-scan credentials',
          description: 'Open the app and scan local CLI configs plus environment variables in one click.',
        },
        {
          step: '03',
          title: 'Watch quota changes',
          description: 'Track burn rate and exhaustion alerts from the dashboard or tray panel.',
        },
      ],
      platforms: [
        {
          name: 'macOS',
          formats: '.dmg / .app',
          note: 'Apple Silicon & Intel',
        },
        {
          name: 'Windows',
          formats: '.exe / .msi',
          note: 'NSIS installer',
        },
        {
          name: 'Linux',
          formats: '.deb / .AppImage',
          note: 'Popular distros',
        },
      ],
      cta: 'Go to GitHub Releases',
    },
    footer: {
      tagline: 'Cross-platform LLM API quota monitoring',
      license: 'MIT License',
    },
  },
  zh: {
    meta: {
      title: 'Last Token — AI API 额度实时监测',
      description:
        'Last Token — 跨平台 LLM / AI API 额度实时监测与耗尽预警。本地优先，隐私安全，避免开发中途断额。',
    },
    nav: {
      features: '功能',
      providers: '提供商',
      download: '下载',
      github: 'GitHub',
      downloadCta: '下载',
    },
    hero: {
      badge: '本地优先 · 隐私安全 · 开源 MIT',
      titleLine1: 'API 额度耗尽，',
      titleHighlight: '不该是惊喜',
      subtitle:
        '跨平台 LLM / AI API 额度实时监测与耗尽预警。自动扫描本地 CLI 凭证，计算消耗速率，在额度见底前发出提醒。',
      download: '免费下载',
      viewSource: '查看源码',
      preview: {
        running: '运行中',
        monitoring: '监测中',
        alerts: '预警',
        resetWindow: '重置周期',
        overview: '额度概览',
        realtime: '实时消耗 · %/小时',
        etaPrefix: '预计耗尽',
        warning: 'Gemini 额度将在 1 小时内耗尽 — 建议切换备用提供商',
      },
    },
    features: {
      eyebrow: 'Features',
      title: '为高频 API 使用者设计',
      subtitle:
        '从发现额度、预测耗尽到及时提醒，Last Token 帮你把「断额」从突发事故变成可预期事件。',
      items: [
        {
          title: '自动扫描 · 一键导入',
          description: '读取本地 CLI 授权配置与 Shell 环境变量，无需手动粘贴密钥，开箱即用。',
        },
        {
          title: '消耗速率与耗尽预测',
          description: '基于时间序列快照计算 %/小时 消耗，并结合重置周期给出精准预警。',
        },
        {
          title: '主界面 + 托盘小组件',
          description: '完整桌面面板与 macOS 状态栏 / 系统托盘并存，额度一眼可见、不打断心流。',
        },
        {
          title: '隐私与安全第一',
          description: 'API 凭证仅存本地内存与 SQLite，绝不上传第三方服务器，本地优先架构。',
        },
        {
          title: '多提供商统一视图',
          description: 'Claude、Copilot、Gemini、Kimi、MiniMax、智谱、火山方舟等配额集中监控。',
        },
        {
          title: '轻量高性能',
          description: 'Tauri 2 + React + Rust 构建，启动快、占用低，适合长期挂机监测。',
        },
      ],
    },
    providers: {
      eyebrow: 'Providers',
      title: '一次看清所有额度',
      subtitle:
        '自动识别本地官方 CLI 授权与环境变量（如 ANTHROPIC_API_KEY、MOONSHOT_API_KEY），统一汇入监测面板。',
      items: [
        { name: 'Claude', detail: 'CLI / Code' },
        { name: 'GitHub Copilot', detail: '配额监测' },
        { name: 'Gemini', detail: 'CLI OAuth' },
        { name: 'Kimi', detail: 'For Coding' },
        { name: 'MiniMax', detail: '编程套餐' },
        { name: '智谱 GLM', detail: '个人 / 团队' },
        { name: '火山方舟', detail: 'Ark' },
        { name: 'ZenMux', detail: '代理路由' },
      ],
    },
    download: {
      eyebrow: 'Download',
      title: '三步开始监测',
      subtitle: '免费开源，跨平台桌面应用。安装后即可开始保护你的 API 额度。',
      steps: [
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
      ],
      platforms: [
        {
          name: 'macOS',
          formats: '.dmg / .app',
          note: 'Apple Silicon & Intel',
        },
        {
          name: 'Windows',
          formats: '.exe / .msi',
          note: 'NSIS 安装包',
        },
        {
          name: 'Linux',
          formats: '.deb / .AppImage',
          note: '主流发行版',
        },
      ],
      cta: '前往 GitHub Releases',
    },
    footer: {
      tagline: '跨平台 LLM API 额度实时监测',
      license: 'MIT License',
    },
  },
} as const

export type Messages = (typeof messages)[Locale]
