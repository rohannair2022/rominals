export default function Home() {
  return (
    <main className="flex min-h-screen items-center justify-center bg-zinc-950 px-6 py-16 text-zinc-50">
      <div className="w-full max-w-5xl rounded-3xl border border-white/10 bg-white/5 p-8 shadow-2xl shadow-black/30 backdrop-blur sm:p-12">
        <div className="mb-10 inline-flex rounded-full border border-emerald-400/30 bg-emerald-400/10 px-4 py-2 text-sm font-medium text-emerald-200">
          Next.js frontend is live
        </div>

        <div className="grid gap-10 lg:grid-cols-[1.4fr_0.9fr]">
          <section className="space-y-6">
            <p className="text-sm font-semibold uppercase tracking-[0.3em] text-zinc-400">
              Rominals
            </p>
            <h1 className="max-w-3xl text-4xl font-semibold tracking-tight text-balance sm:text-6xl">
              A frontend shell for your market data product.
            </h1>
            <p className="max-w-2xl text-lg leading-8 text-zinc-300">
              This Next.js app is ready to become the UI for quotes, watchlists,
              screeners, and portfolio views backed by your Rust services.
            </p>

            <div className="flex flex-wrap gap-3 text-sm text-zinc-200">
              <span className="rounded-full border border-white/10 bg-white/5 px-4 py-2">
                App Router
              </span>
              <span className="rounded-full border border-white/10 bg-white/5 px-4 py-2">
                TypeScript
              </span>
              <span className="rounded-full border border-white/10 bg-white/5 px-4 py-2">
                Tailwind CSS
              </span>
              <span className="rounded-full border border-white/10 bg-white/5 px-4 py-2">
                ESLint
              </span>
            </div>
          </section>

          <aside className="rounded-2xl border border-white/10 bg-black/20 p-6">
            <h2 className="text-lg font-semibold text-white">Start here</h2>
            <ol className="mt-4 space-y-4 text-sm leading-7 text-zinc-300">
              <li>
                Run <code className="rounded bg-white/10 px-2 py-1">npm run dev</code>{" "}
                in <code className="rounded bg-white/10 px-2 py-1">frontend_rominals</code>.
              </li>
              <li>
                Open <code className="rounded bg-white/10 px-2 py-1">http://localhost:3000</code>.
              </li>
              <li>
                Replace this starter page with quote search, watchlists, and
                dashboards when the backend API is ready.
              </li>
            </ol>
          </aside>
        </div>
      </div>
    </main>
  );
}
