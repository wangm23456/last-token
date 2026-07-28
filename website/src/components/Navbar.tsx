import { RELEASES_URL, REPO_URL } from '../lib/utils'

const links = [
  { href: '#features', label: '功能' },
  { href: '#providers', label: '提供商' },
  { href: '#download', label: '下载' },
]

export function Navbar() {
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
          <a
            href={REPO_URL}
            target="_blank"
            rel="noreferrer"
            className="hidden rounded-md px-3 py-1.5 text-sm text-muted-foreground transition-colors duration-150 hover:text-foreground sm:inline-flex"
          >
            GitHub
          </a>
          <a
            href={RELEASES_URL}
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center rounded-lg bg-primary px-3.5 py-1.5 text-sm font-medium text-primary-foreground transition-transform duration-150 hover:opacity-90 active:scale-[0.97]"
          >
            下载
          </a>
        </div>
      </div>
    </header>
  )
}
