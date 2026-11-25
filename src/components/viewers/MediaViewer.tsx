import { useEffect, useState } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { mediaAllow } from '../../lib/ipc';
import { copyText } from '../../lib/clipboard';
import { toMessage } from '../../lib/errors';
import { extOf } from '../../lib/paths';
import { useUi } from '../../store/ui';

type Media = 'image' | 'audio' | 'video' | 'pdf';

function mediaOf(path: string): Media {
  const e = (extOf(path) ?? '').toLowerCase();
  if (e === 'pdf') return 'pdf';
  if (['mp3', 'wav', 'ogg', 'oga', 'flac', 'm4a', 'aac', 'opus'].includes(e)) return 'audio';
  if (['mp4', 'webm', 'mov', 'm4v', 'ogv', 'mkv'].includes(e)) return 'video';
  return 'image';
}

/** Linux (WebKitGTK) no renderiza PDF en iframe. */
const pdfUnsupported = typeof navigator !== 'undefined' && /Linux/i.test(navigator.userAgent) && !/Android/i.test(navigator.userAgent);

/** Imagen, audio, vídeo y PDF por asset protocol (streaming con Range, sin leer el archivo en RAM). */
export function MediaViewer({ path }: { path: string }) {
  const [src, setSrc] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const kind = mediaOf(path);

  useEffect(() => {
    let cancelled = false;
    setSrc(null);
    setError(null);
    mediaAllow(path)
      .then((p) => {
        if (!cancelled) setSrc(convertFileSrc(p));
      })
      .catch((e) => {
        if (!cancelled) setError(toMessage(e));
      });
    return () => {
      cancelled = true;
    };
  }, [path]);

  if (error) return <Message title="No se puede mostrar el archivo" text={error} />;
  if (!src) return <Message title="Cargando…" text="" />;

  if (kind === 'pdf') {
    if (pdfUnsupported) {
      return (
        <Message title="Este sistema no puede mostrar PDF dentro de la app" text={path}>
          <button
            type="button"
            onClick={() => {
              copyText(path)
                .then(() => useUi.getState().pushToast('Ruta copiada: ábrela con tu lector de PDF'))
                .catch((e) => useUi.getState().pushToast(toMessage(e)));
            }}
            className="mt-2 rounded-md border border-border-strong px-3 py-1 text-ui text-fg hover:bg-hover"
          >
            Abrir fuera: copiar ruta
          </button>
        </Message>
      );
    }
    return <iframe title={path} src={src} className="h-full w-full border-0 bg-panel" />;
  }
  if (kind === 'video') {
    return (
      <div className="flex h-full items-center justify-center bg-canvas">
        <video src={src} controls preload="metadata" className="max-h-full max-w-full" />
      </div>
    );
  }
  if (kind === 'audio') {
    return (
      <div className="flex h-full items-center justify-center bg-panel">
        <audio src={src} controls preload="metadata" className="w-[480px] max-w-[90%]" />
      </div>
    );
  }
  return (
    <div className="flex h-full items-center justify-center overflow-auto bg-canvas p-4">
      <img src={src} alt={path} className="max-h-full max-w-full object-contain" />
    </div>
  );
}

function Message({ title, text, children }: { title: string; text: string; children?: React.ReactNode }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-1 bg-base">
      <div className="text-ui-lg text-fg">{title}</div>
      {text && <div className="max-w-[80%] truncate font-mono text-[11px] text-muted">{text}</div>}
      {children}
    </div>
  );
}
