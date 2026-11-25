/**
 * Datos de color NO temáticos. Los colores de UI viven en src/theme/themes.ts como variables
 * CSS --cc-* (cambio de tema en vivo). Aquí solo quedan colores semánticos fijos (iconos de
 * carpeta, servicios, lanes de Git) que no deben cambiar con el tema.
 */
export { currentTheme } from '../theme/applyTheme';

export const palette = {
  accent: '#f59e0b',
  sky: '#38bdf8',
  green: '#10b981',
  warm: '#eab308',
  violet: '#a78bfa',
  excluded: '#52525b',
  /** Colores de servicio en el visor de logs (datos, rotan por hash). */
  serviceColors: ['#60a5fa', '#c084fc', '#34d399', '#f472b6', '#fbbf24', '#22d3ee', '#a3e635', '#fb923c'],
} as const;

export type PaletteKey = keyof typeof palette;

/** Colores de lane del grafo Git (ámbar primero). */
export const laneColors = ['#f59e0b', '#38bdf8', '#a78bfa', '#10b981', '#f472b6', '#eab308', '#fb923c', '#22d3ee'] as const;
