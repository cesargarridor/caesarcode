import { Minus, Plus } from 'lucide-react';
import { IconButton } from './IconButton';
import { TextInput } from './TextInput';

export interface EnvRow {
  k: string;
  v: string;
}

export const rowsFromEnv = (env: Record<string, string> | undefined): EnvRow[] =>
  Object.entries(env ?? {}).map(([k, v]) => ({ k, v }));

export function envFromRows(rows: EnvRow[]): Record<string, string> {
  const out: Record<string, string> = {};
  for (const r of rows) if (r.k.trim()) out[r.k.trim()] = r.v;
  return out;
}

interface Props {
  rows: EnvRow[];
  onRows: (rows: EnvRow[]) => void;
  envFile: string;
  onEnvFile: (v: string) => void;
}

/** Editor de variables de entorno: archivo .env (relativo al directorio) + filas clave/valor. */
export function EnvEditor({ rows, onRows, envFile, onEnvFile }: Props) {
  return (
    <>
      <label className="block">
        <span className="mb-1 block text-ui text-muted">Archivo .env (relativo al directorio)</span>
        <TextInput className="font-mono" value={envFile} onChange={(e) => onEnvFile(e.target.value)} placeholder=".env" />
      </label>
      <div>
        <div className="mb-1 flex items-center justify-between">
          <span className="text-ui text-muted">Variables de entorno</span>
          <IconButton label="Añadir variable" onClick={() => onRows([...rows, { k: '', v: '' }])}>
            <Plus size={14} />
          </IconButton>
        </div>
        <div className="space-y-1">
          {rows.map((r, i) => (
            <div key={i} className="flex items-center gap-1">
              <TextInput
                className="w-2/5 font-mono"
                placeholder="CLAVE"
                value={r.k}
                onChange={(e) => onRows(rows.map((x, j) => (j === i ? { ...x, k: e.target.value } : x)))}
              />
              <TextInput
                className="font-mono"
                placeholder="valor"
                value={r.v}
                onChange={(e) => onRows(rows.map((x, j) => (j === i ? { ...x, v: e.target.value } : x)))}
              />
              <IconButton label="Quitar variable" onClick={() => onRows(rows.filter((_, j) => j !== i))}>
                <Minus size={14} />
              </IconButton>
            </div>
          ))}
        </div>
      </div>
    </>
  );
}
