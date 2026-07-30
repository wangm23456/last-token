import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Toaster } from "@/components/ui/sonner";
import App from "./App";
import "./i18n";
import "./index.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      retry: false,
    },
  },
});

class ErrorBoundary extends React.Component<
  { children: React.ReactNode; t: (key: string) => string },
  { hasError: boolean; error: Error | null }
> {
  constructor(props: { children: React.ReactNode; t: (key: string) => string }) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error) {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error("Uncaught error:", error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="p-4 bg-background text-foreground min-h-screen text-xs space-y-2">
          <h1 className="font-bold text-sm text-status-danger">{this.props.t("error.boundaryTitle")}</h1>
          <p className="text-muted-foreground">{this.state.error?.message || this.props.t("error.unknownError")}</p>
          <pre className="p-2 bg-card border border-border text-[10px] overflow-auto max-h-40 rounded">
            {this.state.error?.stack}
          </pre>
          <button
            type="button"
            className="px-3 py-1 bg-primary text-primary-foreground rounded text-xs"
            onClick={() => window.location.reload()}
          >
            {this.props.t("error.reload")}
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}

function ErrorBoundaryWithI18n({ children }: { children: React.ReactNode }) {
  const { t } = useTranslation();
  return <ErrorBoundary t={t}>{children}</ErrorBoundary>;
}

const rootElement = document.getElementById("root");

if (!rootElement) {
  throw new Error("Missing #root mount element");
}

ReactDOM.createRoot(rootElement).render(
  <React.StrictMode>
    <ErrorBoundaryWithI18n>
      <QueryClientProvider client={queryClient}>
        <TooltipProvider>
          <App />
          <Toaster closeButton position="bottom-right" theme="dark" />
        </TooltipProvider>
      </QueryClientProvider>
    </ErrorBoundaryWithI18n>
  </React.StrictMode>,
);
