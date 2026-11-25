import { Component, type ReactNode } from 'react';

/** Evita pantalla negra: muestra el error y permite reintentar. */
export class ErrorBoundary extends Component<{ children: ReactNode; label?: string }, { err: Error | null }> {
  state = { err: null as Error | null };
  static getDerivedStateFromError(err: Error) { return { err }; }
  render() {
    if (!this.state.err) return this.props.children;
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 p-6 text-center">
        <div className="text-ui-lg text-fg">{this.props.label ?? 'Algo falló'}</div>
        <pre className="max-w-full overflow-auto text-[11px] text-muted">{this.state.err.message}</pre>
        <button type="button" className="rounded-md bg-elevated px-3 py-1 text-ui text-fg hover:bg-elevated-hover" onClick={() => this.setState({ err: null })}>Reintentar</button>
      </div>
    );
  }
}
