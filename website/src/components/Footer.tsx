import { REPO_URL } from '../lib/utils'

export function Footer() {
  return (
    <footer className="border-t border-border/50 py-10">
      <div className="mx-auto flex max-w-6xl flex-col items-center justify-between gap-4 px-4 sm:flex-row sm:px-6">
        <div className="flex items-center gap-2.5">
          <img
            src="/last-token.png"
            alt="Last Token"
            className="h-6 w-6 rounded object-contain"
          />
          <div>
            <div className="text-sm font-semibold tracking-tight">Last Token</div>
            <div className="text-xs text-muted-foreground">
              跨平台 LLM API 额度实时监测
            </div>
          </div>
        </div>

        <div className="flex items-center gap-4 text-xs text-muted-foreground">
          <a
            href={REPO_URL}
            target="_blank"
            rel="noreferrer"
            className="transition-colors hover:text-foreground"
          >
            GitHub
          </a>
          <a
            href={`${REPO_URL}/blob/main/LICENSE`}
            target="_blank"
            rel="noreferrer"
            className="transition-colors hover:text-foreground"
          >
            MIT License
          </a>
          <span>© {new Date().getFullYear()}</span>
        </div>
      </div>
    </footer>
  )
}
