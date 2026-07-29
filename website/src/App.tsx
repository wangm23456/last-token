import { LocaleProvider } from './i18n/LocaleContext'
import { Navbar } from './components/Navbar'
import { Hero } from './components/Hero'
import { Features } from './components/Features'
import { Providers } from './components/Providers'
import { Download } from './components/Download'
import { Footer } from './components/Footer'

export default function App() {
  return (
    <LocaleProvider>
      <div className="min-h-screen">
        <Navbar />
        <main>
          <Hero />
          <Features />
          <Providers />
          <Download />
        </main>
        <Footer />
      </div>
    </LocaleProvider>
  )
}
