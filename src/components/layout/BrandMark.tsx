import brand from '../../assets/brand.png';

/** Marca: el mismo icono imperial de la app. */
export function BrandMark({ size = 16, className }: { size?: number; className?: string }) {
  return (
    <img
      src={brand}
      width={size}
      height={size}
      className={className}
      style={{ imageRendering: 'pixelated' }}
      alt=""
      aria-hidden="true"
      draggable={false}
    />
  );
}
