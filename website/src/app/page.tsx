export default function Home() {
  return (
    <div className="min-h-screen bg-neutral-950 text-neutral-50 font-sans selection:bg-cyan-500/30">
      {/* Hero Section */}
      <main className="relative flex flex-col items-center justify-center min-h-screen px-4 overflow-hidden">
        
        {/* Background Gradients */}
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[800px] h-[800px] bg-cyan-500/10 blur-[120px] rounded-full pointer-events-none" />
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[400px] h-[400px] bg-blue-500/20 blur-[100px] rounded-full pointer-events-none" />

        <div className="z-10 flex flex-col items-center text-center space-y-8 max-w-4xl mx-auto mt-20">
          <div className="inline-flex items-center px-3 py-1 rounded-full border border-white/10 bg-white/5 backdrop-blur-md text-sm text-neutral-300 font-medium">
            ✨ v0.1.5 is now live on npm
          </div>
          
          <h1 className="text-5xl md:text-7xl font-bold tracking-tight bg-clip-text text-transparent bg-gradient-to-b from-white to-white/60">
            The Missing Control Center <br/>
            for Linux Power Users
          </h1>
          
          <p className="text-lg md:text-xl text-neutral-400 max-w-2xl leading-relaxed">
            A hyper-fast, native TUI settings dashboard that binds directly to Wayland, D-Bus, and systemd. Zero bloat. 100% terminal.
          </p>

          <div className="flex items-center gap-4 pt-4">
            <code className="px-6 py-4 rounded-xl bg-neutral-900 border border-white/10 text-cyan-400 font-mono shadow-2xl">
              npm install -g settings-tui
            </code>
          </div>
        </div>
      </main>
    </div>
  );
}
